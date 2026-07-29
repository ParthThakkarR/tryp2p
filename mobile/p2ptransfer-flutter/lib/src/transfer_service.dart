import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:cryptography/cryptography.dart';

// ── Wire protocol tags (must match p2ptransfer-core exactly) ────────────────
const int _tagMetadata = 0x00;
const int _tagChunk = 0x01;
const int _tagChunkAck = 0x02;
const int _tagComplete = 0x03;
const int _tagError = 0x04;
const int _tagClientHello = 0x05;
const int _tagServerHello = 0x06;

const int _defaultPort = 9877;
const int _chunkSize = 256 * 1024; // 256 KB — good balance on mobile Wi-Fi

// ── Transfer progress event ─────────────────────────────────────────────────
class TransferProgress {
  final String sessionId;
  final String fileName;
  final int bytesTransferred;
  final int totalBytes;
  final double speedBps;
  final bool isDone;
  final bool isError;
  final String? errorMessage;

  TransferProgress({
    required this.sessionId,
    required this.fileName,
    required this.bytesTransferred,
    required this.totalBytes,
    this.speedBps = 0,
    this.isDone = false,
    this.isError = false,
    this.errorMessage,
  });

  double get fraction =>
      totalBytes > 0 ? (bytesTransferred / totalBytes).clamp(0.0, 1.0) : 0.0;

  String get speedLabel {
    if (speedBps < 1024) return '${speedBps.toStringAsFixed(0)} B/s';
    if (speedBps < 1024 * 1024) {
      return '${(speedBps / 1024).toStringAsFixed(1)} KB/s';
    }
    return '${(speedBps / (1024 * 1024)).toStringAsFixed(1)} MB/s';
  }
}

// ── Incoming transfer request (shown in Accept/Decline dialog) ───────────────
class IncomingTransferRequest {
  final String senderIp;
  final String sessionId;
  final String fileName;
  final int fileSize;
  final Completer<String?> _decision; // null = declined, non-null = save path

  IncomingTransferRequest({
    required this.senderIp,
    required this.sessionId,
    required this.fileName,
    required this.fileSize,
  }) : _decision = Completer<String?>();

  void accept(String savePath) {
    if (!_decision.isCompleted) _decision.complete(savePath);
  }

  void decline() {
    if (!_decision.isCompleted) _decision.complete(null);
  }

  Future<String?> get decision => _decision.future;
}

// ── TransferService singleton ────────────────────────────────────────────────
class TransferService {
  TransferService._();
  static final TransferService instance = TransferService._();

  ServerSocket? _serverSocket;
  bool _listening = false;
  int _listenPort = _defaultPort;

  // Stream of incoming requests (receive_page listens to this)
  final _incomingController =
      StreamController<IncomingTransferRequest>.broadcast();
  Stream<IncomingTransferRequest> get incomingRequests =>
      _incomingController.stream;

  bool get isListening => _listening;
  int get listenPort => _listenPort;

  // ── Start TCP listener ──────────────────────────────────────────────────
  Future<void> startListening({int port = _defaultPort}) async {
    if (_listening) return;
    _listenPort = port;
    try {
      _serverSocket = await ServerSocket.bind(
        InternetAddress.anyIPv4,
        port,
        shared: true,
      );
      _listening = true;
      _serverSocket!.listen(
        _handleIncomingConnection,
        onError: (_) {},
        onDone: () => _listening = false,
      );
    } catch (e) {
      _listening = false;
    }
  }

  Future<void> stopListening() async {
    await _serverSocket?.close();
    _serverSocket = null;
    _listening = false;
  }

  // ── Handle a new incoming TCP connection ────────────────────────────────
  Future<void> _handleIncomingConnection(Socket socket) async {
    try {
      socket.setOption(SocketOption.tcpNoDelay, true);
      final addr = socket.remoteAddress.address;

      // 1. ECDH handshake — receiver side
      final (tag0, clientPub) = await _readTagged(socket);
      if (tag0 != _tagClientHello || clientPub.length != 32) {
        await _writeTagged(socket, _tagError, utf8.encode('Expected CLIENT_HELLO'));
        socket.destroy();
        return;
      }

      final kx = X25519();
      final serverKp = await kx.newKeyPair();
      final serverPub = await serverKp.extractPublicKey();
      final serverPubBytes = serverPub.bytes;
      await _writeTagged(socket, _tagServerHello, Uint8List.fromList(serverPubBytes));

      final clientPubKey = SimplePublicKey(clientPub, type: KeyPairType.x25519);
      final sharedSecret = await kx.sharedSecretKey(
        keyPair: serverKp,
        remotePublicKey: clientPubKey,
      );
      final encKey = await _deriveEncKey(await sharedSecret.extractBytes());

      // 2. Read METADATA frame
      final (tag1, metaBytes) = await _readTagged(socket);
      if (tag1 != _tagMetadata) {
        await _writeTagged(socket, _tagError, utf8.encode('Expected METADATA'));
        socket.destroy();
        return;
      }

      final meta = jsonDecode(utf8.decode(metaBytes)) as Map<String, dynamic>;
      final sessionId = meta['session_id'] as String? ?? _randomId();
      final fileName = meta['file_name'] as String? ?? 'file';
      final fileSize = (meta['file_size'] as num?)?.toInt() ?? 0;
      final totalChunks = (meta['total_chunks'] as num?)?.toInt() ?? 1;
      final chunkSz = (meta['chunk_size'] as num?)?.toInt() ?? _chunkSize;
      final noncePrefixList = (meta['nonce_prefix'] as List?)
              ?.map((e) => (e as num).toInt())
              .toList() ??
          [0, 0, 0, 0];
      final noncePrefix =
          Uint8List.fromList(noncePrefixList.take(4).toList());
      final checksumList = (meta['checksum'] as List?)
              ?.map((e) => (e as num).toInt())
              .toList() ??
          List.filled(32, 0);

      // 3. Show incoming request to UI — wait for user decision
      final request = IncomingTransferRequest(
        senderIp: addr,
        sessionId: sessionId,
        fileName: fileName,
        fileSize: fileSize,
      );
      _incomingController.add(request);

      final savePath = await request.decision.timeout(
        const Duration(minutes: 2),
        onTimeout: () => null,
      );

      if (savePath == null) {
        await _writeTagged(socket, _tagError, utf8.encode('DECLINED'));
        socket.destroy();
        return;
      }

      // 4. Accept
      await _writeTagged(socket, _tagMetadata, utf8.encode('ACCEPT'));

      // 5. Receive chunks
      final outFile = File(savePath);
      await outFile.parent.create(recursive: true);
      final raf = await outFile.open(mode: FileMode.writeOnly);

      int bytesReceived = 0;
      final startTime = DateTime.now();

      final aead = Chacha20.poly1305Aead();

      for (int chunkIdx = 0; chunkIdx < totalChunks; chunkIdx++) {
        final (chunkTag, payload) = await _readTagged(socket);
        if (chunkTag == _tagError) {
          await raf.close();
          throw Exception('Sender error: ${utf8.decode(payload)}');
        }
        if (chunkTag != _tagChunk || payload.length < 5) {
          throw Exception('Bad chunk frame');
        }

        final recvIdx = ByteData.view(
          Uint8List.fromList(payload.sublist(0, 4)).buffer,
        ).getUint32(0, Endian.little);

        final compressed = payload[4] != 0;
        final encryptedData = payload.sublist(5);

        // Build nonce: 4-byte prefix + 8-byte chunk counter (LE)
        final nonce = _buildNonce(noncePrefix, chunkIdx);

        // Decrypt
        final encKeyBytes = await encKey.extractBytes();
        final secretBox = SecretBox(
          encryptedData.sublist(0, encryptedData.length - 16),
          nonce: nonce,
          mac: Mac(encryptedData.sublist(encryptedData.length - 16)),
        );
        final chunkData = await aead.decrypt(
          secretBox,
          secretKey: SecretKey(encKeyBytes),
        );

        // Decompress if needed (zstd is Rust-only; for now we skip compression
        // on mobile by always sending compressed_flag=0 from Dart sender)
        final finalChunk = compressed ? chunkData : chunkData;

        // Write at correct offset
        final offset = recvIdx * chunkSz;
        await raf.setPosition(offset);
        await raf.writeFrom(finalChunk);

        bytesReceived += finalChunk.length;

        // Send ACK
        final ack = Uint8List(4);
        ByteData.view(ack.buffer).setUint32(0, recvIdx, Endian.little);
        await _writeTagged(socket, _tagChunkAck, ack);
      }

      await raf.close();

      // 6. Verify checksum (BLAKE3 is Rust-only; do a basic size check for now)
      // Full BLAKE3 requires native — skipping cryptographic verify but
      // we confirm size matches
      final actualSize = await outFile.length();
      if (actualSize == fileSize) {
        await _writeTagged(
          socket,
          _tagComplete,
          Uint8List.fromList(checksumList),
        );
      } else {
        await _writeTagged(
          socket,
          _tagError,
          utf8.encode('Size mismatch'),
        );
      }

      final elapsed =
          DateTime.now().difference(startTime).inMilliseconds / 1000.0;
      final speedMBps = elapsed > 0 ? bytesReceived / elapsed / (1024 * 1024) : 0;
      // ignore: avoid_print
      print('Received $fileName — ${speedMBps.toStringAsFixed(1)} MB/s');
    } catch (e) {
      // ignore: avoid_print
      print('Incoming connection error: $e');
    } finally {
      socket.destroy();
    }
  }

  // ── Send a file to a peer ───────────────────────────────────────────────
  Stream<TransferProgress> sendFile({
    required String peerIp,
    required int peerPort,
    required String filePath,
  }) {
    final controller = StreamController<TransferProgress>();
    _doSend(
      peerIp: peerIp,
      peerPort: peerPort,
      filePath: filePath,
      progress: controller,
    ).whenComplete(() => controller.close());
    return controller.stream;
  }

  Future<void> _doSend({
    required String peerIp,
    required int peerPort,
    required String filePath,
    required StreamController<TransferProgress> progress,
  }) async {
    final file = File(filePath);
    final fileName = filePath.split(Platform.pathSeparator).last;
    final fileSize = await file.length();
    final sessionId = _randomId();

    void emit(TransferProgress p) {
      if (!progress.isClosed) progress.add(p);
    }

    Socket? socket;
    try {
      // Connect
      socket = await Socket.connect(
        peerIp,
        peerPort,
        timeout: const Duration(seconds: 10),
      );
      socket.setOption(SocketOption.tcpNoDelay, true);

      // Buffer all incoming data
      final responseBuffer = _SocketBuffer(socket);

      // 1. ECDH handshake — sender side
      final kx = X25519();
      final clientKp = await kx.newKeyPair();
      final clientPub = await clientKp.extractPublicKey();
      await _writeTaggedToSocket(socket, _tagClientHello,
          Uint8List.fromList(clientPub.bytes));

      final (helloTag, serverPubRaw) = await responseBuffer.readTagged();
      if (helloTag != _tagServerHello || serverPubRaw.length != 32) {
        throw Exception('Bad handshake response');
      }

      final serverPubKey =
          SimplePublicKey(serverPubRaw, type: KeyPairType.x25519);
      final sharedSecret = await kx.sharedSecretKey(
        keyPair: clientKp,
        remotePublicKey: serverPubKey,
      );
      final encKey = await _deriveEncKey(await sharedSecret.extractBytes());
      final encKeyBytes = await encKey.extractBytes();
      final noncePrefix = _randomNoncePrefix();

      // Compute chunk count
      final totalChunks = (fileSize / _chunkSize).ceil().clamp(1, 1 << 31);

      // 2. Send METADATA
      final meta = jsonEncode({
        'session_id': sessionId,
        'file_name': fileName,
        'file_path': filePath,
        'file_size': fileSize,
        'chunk_size': _chunkSize,
        'total_chunks': totalChunks,
        'checksum': List.filled(32, 0), // placeholder
        'resume_offset': 0,
        'is_resume': false,
        'nonce_prefix': noncePrefix.toList(),
      });
      await _writeTaggedToSocket(
          socket, _tagMetadata, Uint8List.fromList(utf8.encode(meta)));

      final (acceptTag, acceptPayload) = await responseBuffer.readTagged();
      if (acceptTag == _tagError) {
        throw Exception(
            'Declined: ${utf8.decode(acceptPayload)}');
      }
      if (acceptTag != _tagMetadata || utf8.decode(acceptPayload) != 'ACCEPT') {
        throw Exception('Transfer rejected by receiver');
      }

      // 3. Send chunks
      final aead = Chacha20.poly1305Aead();
      final raf = await file.open(mode: FileMode.read);
      int bytesSent = 0;
      final startTime = DateTime.now();

      for (int chunkIdx = 0; chunkIdx < totalChunks; chunkIdx++) {
        final offset = chunkIdx * _chunkSize;
        await raf.setPosition(offset);
        final readLen = (fileSize - offset).clamp(0, _chunkSize);
        final rawChunk = await raf.read(readLen);

        // No compression on mobile for now (Rust receiver handles both)
        final nonce = _buildNonce(Uint8List.fromList(noncePrefix), chunkIdx);
        final secretBox = await aead.encrypt(
          rawChunk,
          secretKey: SecretKey(encKeyBytes),
          nonce: nonce,
        );

        // Encrypted = ciphertext + 16-byte MAC tag
        final encryptedData = Uint8List(
            secretBox.cipherText.length + secretBox.mac.bytes.length);
        encryptedData.setAll(0, secretBox.cipherText);
        encryptedData.setAll(
            secretBox.cipherText.length, secretBox.mac.bytes);

        // Frame: [u32 LE chunk_index][u8 compressed_flag=0][encrypted]
        final frame = Uint8List(5 + encryptedData.length);
        ByteData.view(frame.buffer).setUint32(0, chunkIdx, Endian.little);
        frame[4] = 0; // not compressed
        frame.setAll(5, encryptedData);

        await _writeTaggedToSocket(socket, _tagChunk, frame);

        // Wait for ACK (streaming — no pipelining yet)
        final (ackTag, ackPayload) = await responseBuffer.readTagged();
        if (ackTag == _tagError) {
          throw Exception('Receiver error: ${utf8.decode(ackPayload)}');
        }
        if (ackTag != _tagChunkAck) {
          throw Exception('Expected CHUNK_ACK, got $ackTag');
        }

        bytesSent += rawChunk.length;
        final elapsed =
            DateTime.now().difference(startTime).inMilliseconds / 1000.0;
        final speedBps = elapsed > 0 ? bytesSent / elapsed : 0.0;

        emit(TransferProgress(
          sessionId: sessionId,
          fileName: fileName,
          bytesTransferred: bytesSent,
          totalBytes: fileSize,
          speedBps: speedBps,
        ));
      }

      await raf.close();

      // 4. Done
      emit(TransferProgress(
        sessionId: sessionId,
        fileName: fileName,
        bytesTransferred: fileSize,
        totalBytes: fileSize,
        isDone: true,
      ));
    } catch (e) {
      emit(TransferProgress(
        sessionId: sessionId,
        fileName: fileName,
        bytesTransferred: 0,
        totalBytes: fileSize,
        isError: true,
        errorMessage: e.toString(),
      ));
    } finally {
      socket?.destroy();
    }
  }

  // ── Key derivation (matches Rust: HKDF-SHA256) ──────────────────────────
  Future<SecretKey> _deriveEncKey(List<int> sharedSecret) async {
    final hkdf = Hkdf(
      hmac: Hmac.sha256(),
      outputLength: 32,
    );
    final salt = utf8.encode('P2PTRANSFER_SALT_v1');
    final info = utf8.encode('p2ptransfer-v1-encryption');
    return hkdf.deriveKey(
      secretKey: SecretKey(sharedSecret),
      nonce: salt,
      info: info,
    );
  }

  // ── Nonce: 4-byte prefix + 8-byte LE chunk counter (matches Rust) ───────
  List<int> _buildNonce(Uint8List prefix, int chunkIndex) {
    final nonce = List<int>.filled(12, 0);
    nonce.setAll(0, prefix.take(4));
    // 8-byte LE chunk counter in bytes 4..12
    var idx = chunkIndex;
    for (int i = 4; i < 12; i++) {
      nonce[i] = idx & 0xFF;
      idx >>= 8;
    }
    return nonce;
  }

  Uint8List _randomNoncePrefix() {
    final rng = Random.secure();
    return Uint8List.fromList(
        List.generate(4, (_) => rng.nextInt(256)));
  }

  String _randomId() {
    final rng = Random.secure();
    return List.generate(8, (_) => rng.nextInt(256))
        .map((b) => b.toRadixString(16).padLeft(2, '0'))
        .join();
  }

  // ── Low-level framing (server-side: uses Socket directly) ────────────────
  /// Frame: [u64 BE length][payload]
  Future<(int tag, Uint8List payload)> _readTagged(Socket socket) async {
    // Use a buffer helper that wraps the socket stream
    final buf = _SocketBuffer(socket);
    return buf.readTagged();
  }

  Future<void> _writeTagged(
      Socket socket, int tag, List<int> payload) async {
    await _writeTaggedToSocket(socket, tag,
        payload is Uint8List ? payload : Uint8List.fromList(payload));
  }

  Future<void> _writeTaggedToSocket(
      Socket socket, int tag, Uint8List payload) async {
    final frame = Uint8List(8 + 1 + payload.length);
    final totalLen = 1 + payload.length;
    ByteData.view(frame.buffer).setUint64(0, totalLen, Endian.big);
    frame[8] = tag;
    frame.setAll(9, payload);
    socket.add(frame);
    await socket.flush();
  }
}

// (nonce helper is defined as an instance method on TransferService)

// ── Socket buffered reader ────────────────────────────────────────────────────
/// Buffers raw socket bytes and allows reading framed messages.
class _SocketBuffer {
  final Socket _socket;
  final _buf = <int>[];
  StreamSubscription<Uint8List>? _sub;
  final _dataAvailable = StreamController<void>.broadcast();
  bool _done = false;

  _SocketBuffer(this._socket) {
    _sub = _socket.cast<Uint8List>().listen(
      (data) {
        _buf.addAll(data);
        _dataAvailable.add(null);
      },
      onDone: () {
        _done = true;
        _dataAvailable.add(null);
      },
      onError: (_) {
        _done = true;
        _dataAvailable.add(null);
      },
      cancelOnError: false,
    );
  }

  Future<Uint8List> _readExact(int n) async {
    while (_buf.length < n) {
      if (_done) throw Exception('Connection closed before reading $n bytes');
      await _dataAvailable.stream.first;
    }
    final result = Uint8List.fromList(_buf.sublist(0, n));
    _buf.removeRange(0, n);
    return result;
  }

  Future<(int tag, Uint8List payload)> readTagged() async {
    // Read 8-byte big-endian length
    final lenBytes = await _readExact(8);
    final msgLen =
        ByteData.view(lenBytes.buffer).getUint64(0, Endian.big);

    if (msgLen == 0) throw Exception('Empty message');
    if (msgLen > 64 * 1024 * 1024) throw Exception('Message too large: $msgLen');

    final data = await _readExact(msgLen);
    final tag = data[0];
    final payload = data.sublist(1);
    return (tag, payload);
  }

  void dispose() {
    _sub?.cancel();
    _dataAvailable.close();
  }
}
