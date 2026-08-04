import 'dart:async';
import 'dart:io';
import 'package:flutter/foundation.dart';

import 'rust/api.dart' as api;

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
  final String? status;

  TransferProgress({
    required this.sessionId,
    required this.fileName,
    required this.bytesTransferred,
    required this.totalBytes,
    this.speedBps = 0,
    this.isDone = false,
    this.isError = false,
    this.errorMessage,
    this.status,
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
  final String senderIp; // Just keeping this for compatibility
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

  bool _listening = false;

  final _incomingController =
      StreamController<IncomingTransferRequest>.broadcast();
  Stream<IncomingTransferRequest> get incomingRequests =>
      _incomingController.stream;

  bool get isListening => _listening;
  int get listenPort => 0; // Legacy

  StreamSubscription<api.FrbTransferEvent>? _backendEventSub;
  final _pendingSavePaths = <String, String>{};
  String? _activeOutgoingRequestId;
  // Per-session progress map for receive side polling
  final _activeProgress = <String, TransferProgress>{};

  String? get activeOutgoingRequestId => _activeOutgoingRequestId;

  /// Returns current progress for a receive-side session, or null if not found.
  TransferProgress? getActiveTransferProgress(String sessionId) {
    return _activeProgress[sessionId];
  }

  /// Returns all active receive-side transfers.
  List<TransferProgress> get activeReceiveTransfers =>
      _activeProgress.values.toList();

  // Initialize the unified rust backend
  Future<void> startListeningWithShortId(String shortId, String defaultOutputDir) async {
    if (_listening) return;
    
    // We start the backend and listen to the event stream
    final eventStream = api.initBackend(shortId: shortId, outputDir: defaultOutputDir);
    
    _backendEventSub = eventStream.listen((event) async {
      event.map(
        sendStatus: (_) {},
        rejected: (_) {},
        error: (e) {
          final prev = _activeProgress[e.requestId];
          _activeProgress[e.requestId] = TransferProgress(
            sessionId: e.requestId,
            fileName: prev?.fileName ?? e.requestId,
            bytesTransferred: prev?.bytesTransferred ?? 0,
            totalBytes: prev?.totalBytes ?? 0,
            isError: true,
            errorMessage: e.error,
          );
        },
        incoming: (inc) async {
          _activeProgress[inc.requestId] = TransferProgress(
            sessionId: inc.requestId,
            fileName: inc.fileName,
            bytesTransferred: 0,
            totalBytes: inc.fileSize.toInt(),
            status: "incoming",
          );

          final request = IncomingTransferRequest(
            senderIp: inc.senderName, // We use senderName for UI display
            sessionId: inc.requestId,
            fileName: inc.fileName,
            fileSize: inc.fileSize.toInt(),
          );
          _incomingController.add(request);
          
          final savePath = await request.decision.timeout(
            const Duration(minutes: 2),
            onTimeout: () => null,
          );
          
          if (savePath == null) {
            await api.respondToTransfer(requestId: inc.requestId, accepted: false);
          } else {
            _pendingSavePaths[inc.requestId] = savePath;
            await api.respondToTransfer(requestId: inc.requestId, accepted: true);
          }
        },
        progress: (prog) {
          // Update per-session progress map for receive-side polling
          final prev = _activeProgress[prog.requestId];
          _activeProgress[prog.requestId] = TransferProgress(
            sessionId: prog.requestId,
            fileName: prev?.fileName ?? prog.requestId,
            bytesTransferred: prog.bytesTransferred.toInt(),
            totalBytes: prog.total.toInt(),
            speedBps: prog.speedBytesPerSec,
            status: "transferring",
          );
        },
        complete: (c) {
          // Mark session as done in progress map
          final prev = _activeProgress[c.requestId];
          _activeProgress[c.requestId] = TransferProgress(
            sessionId: c.requestId,
            fileName: prev?.fileName ?? c.requestId,
            bytesTransferred: prev?.totalBytes ?? 0,
            totalBytes: prev?.totalBytes ?? 0,
            isDone: true,
          );
          final targetDir = _pendingSavePaths.remove(c.requestId);
          if (targetDir != null) {
            try {
              final sourceFile = File(c.filePath);
              final fileName = sourceFile.uri.pathSegments.last;
              final targetFile = File('$targetDir${Platform.pathSeparator}$fileName');
              if (sourceFile.existsSync() && sourceFile.path != targetFile.path) {
                sourceFile.renameSync(targetFile.path);
              }
            } catch (e) {
              debugPrint('Failed to move file: $e');
            }
          }
        },
        cancelled: (c) {
          final prev = _activeProgress[c.requestId];
          _activeProgress[c.requestId] = TransferProgress(
            sessionId: c.requestId,
            fileName: prev?.fileName ?? c.requestId,
            bytesTransferred: prev?.bytesTransferred ?? 0,
            totalBytes: prev?.totalBytes ?? 0,
            isError: true,
            errorMessage: 'Transfer cancelled by user',
          );
        },
      );
    });
    
    _listening = true;
  }

  Future<void> stopListening() async {
    await _backendEventSub?.cancel();
    _backendEventSub = null;
    _listening = false;
  }

  /// Pause an active outgoing transfer.
  Future<void> pauseTransfer(String requestId) async {
    await api.pauseTransfer(requestId: requestId);
  }

  /// Resume a paused outgoing transfer.
  Future<void> resumeTransfer(String requestId) async {
    await api.resumeTransfer(requestId: requestId);
  }

  /// Cancel an active transfer (outgoing or incoming).
  Future<void> cancelTransfer(String requestId) async {
    await api.cancelTransfer(requestId: requestId);
    _activeOutgoingRequestId = null;
  }

  // ── Send a file to a peer ───────────────────────────────────────────────
  Stream<TransferProgress> sendFile({
    required String peerShortId, // Note: peerIp becomes peerShortId
    required String filePath,
  }) {
    final controller = StreamController<TransferProgress>();
    final requestId = api.generateRandomShortId();
    _activeOutgoingRequestId = requestId;
    
    final file = File(filePath);
    final fileName = filePath.split(Platform.pathSeparator).last;
    final fileSize = file.lengthSync();
    
    // Call the Rust async function which returns a stream of events
    final stream = api.sendFile(
      requestId: requestId,
      targetShortId: peerShortId,
      filePath: filePath,
      senderName: "Mobile",
    );
    
    stream.listen((event) {
      event.map(
        sendStatus: (s) {
          controller.add(TransferProgress(
            sessionId: requestId,
            fileName: fileName,
            bytesTransferred: 0,
            totalBytes: fileSize,
            status: s.status,
          ));
        },
        rejected: (e) {
          controller.add(TransferProgress(
            sessionId: requestId,
            fileName: fileName,
            bytesTransferred: 0,
            totalBytes: fileSize,
            isError: true,
            errorMessage: "Transfer rejected by receiver",
          ));
          controller.close();
        },
        error: (e) {
          controller.add(TransferProgress(
            sessionId: requestId,
            fileName: fileName,
            bytesTransferred: 0,
            totalBytes: fileSize,
            isError: true,
            errorMessage: e.error,
          ));
          controller.close();
        },
        incoming: (_) {},
        progress: (p) {
          controller.add(TransferProgress(
            sessionId: requestId,
            fileName: fileName,
            bytesTransferred: p.bytesTransferred.toInt(),
            totalBytes: p.total.toInt(),
            speedBps: p.speedBytesPerSec,
          ));
        },
        complete: (c) {
          controller.add(TransferProgress(
            sessionId: requestId,
            fileName: fileName,
            bytesTransferred: fileSize,
            totalBytes: fileSize,
            isDone: true,
          ));
          controller.close();
        },
        cancelled: (_) {
          _activeOutgoingRequestId = null;
          controller.add(TransferProgress(
            sessionId: requestId,
            fileName: fileName,
            bytesTransferred: 0,
            totalBytes: fileSize,
            isError: true,
            errorMessage: 'Transfer cancelled by user',
            status: 'cancelled',
          ));
          controller.close();
        },
      );
    }, onError: (e) {
      _activeOutgoingRequestId = null;
      controller.add(TransferProgress(
        sessionId: requestId,
        fileName: fileName,
        bytesTransferred: 0,
        totalBytes: fileSize,
        isError: true,
        errorMessage: e.toString(),
      ));
      controller.close();
    });
    
    return controller.stream;
  }
}
