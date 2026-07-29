import 'dart:async';
import 'dart:convert';
import 'dart:io';
import '../main.dart';
import 'settings_page.dart';
import 'contacts_service.dart';

class DiscoveredPeer {
  final String deviceName;
  final String address;
  final int port;
  final String? deviceId; // short XXXX-XXXX key
  final DateTime lastSeen;

  DiscoveredPeer({
    required this.deviceName,
    required this.address,
    required this.port,
    this.deviceId,
    required this.lastSeen,
  });

  String get fullAddress => '$address:$port';
}

class PeerDiscoveryService {
  static const int discoveryPort = 9876;
  static const String multicastAddress = '224.0.0.251';

  RawDatagramSocket? _socket;
  Timer? _beaconTimer;

  final _peersMap = <String, DiscoveredPeer>{};
  final _controller = StreamController<List<DiscoveredPeer>>.broadcast();

  Stream<List<DiscoveredPeer>> get peersStream => _controller.stream;
  List<DiscoveredPeer> get currentPeers => _peersMap.values.toList();

  /// Map from normalised short device key → peer (for send-page key lookup).
  Map<String, DiscoveredPeer> get keyToPeer {
    final map = <String, DiscoveredPeer>{};
    for (final peer in _peersMap.values) {
      if (peer.deviceId != null) {
        final normalized = peer.deviceId!.replaceAll('-', '').toUpperCase();
        map[normalized] = peer;
      }
    }
    return map;
  }

  Future<void> startDiscovery() async {
    try {
      _socket = await RawDatagramSocket.bind(
        InternetAddress.anyIPv4,
        discoveryPort,
        reuseAddress: true,
        reusePort: true,
      );

      _socket?.multicastLoopback = false;
      try {
        _socket?.joinMulticast(InternetAddress(multicastAddress));
      } catch (_) {}

      _socket?.listen((RawSocketEvent event) {
        if (event == RawSocketEvent.read) {
          final datagram = _socket?.receive();
          if (datagram != null) _handleDatagram(datagram);
        }
      });

      // Send beacon immediately, then every 2 s to stay fresh
      _sendBeacon();
      _beaconTimer = Timer.periodic(const Duration(seconds: 2), (_) {
        _sendBeacon();
      });
    } catch (_) {}
  }

  void _sendBeacon() {
    try {
      final beaconData = jsonEncode({
        'device_name': AppSettings.deviceName.isNotEmpty
            ? AppSettings.deviceName
            : Platform.localHostname,
        'p2ptransfer_version': '1.0.0',
        'tcp_port': AppSettings.tcpPort,
        // Broadcast our short key so remote peers can display / look up
        'device_id':
            deviceIdentity.isInitialized ? deviceIdentity.deviceId : null,
      });
      final bytes = utf8.encode(beaconData);
      _socket?.send(bytes, InternetAddress(multicastAddress), discoveryPort);
      _socket?.send(bytes, InternetAddress('255.255.255.255'), discoveryPort);
    } catch (_) {}
  }

  void _handleDatagram(Datagram datagram) {
    try {
      final text = utf8.decode(datagram.data);
      final json = jsonDecode(text) as Map<String, dynamic>;

      final deviceName = json['device_name'] as String? ?? 'Unknown Peer';
      final port = json['tcp_port'] as int? ?? 9877;
      final rawKey = json['device_id'] as String?;
      // Normalise key to uppercase XXXX-XXXX
      final deviceId = rawKey != null ? _normaliseKey(rawKey) : null;
      final senderIp = datagram.address.address;

      final peerKey = '$senderIp:$port';
      _peersMap[peerKey] = DiscoveredPeer(
        deviceName: deviceName,
        address: senderIp,
        port: port,
        deviceId: deviceId,
        lastSeen: DateTime.now(),
      );

      // Update last-known IP in contacts if this peer is a saved contact
      if (deviceId != null) {
        ContactsService.instance.updateIp(deviceId, senderIp, port);
      }

      _controller.add(currentPeers);
    } catch (_) {}
  }

  /// Resolve a raw key input (e.g. "A3F8-K2D1" or "A3F8K2D1") to a peer.
  /// First tries LAN discovery, then contacts service (uses last-known IP).
  DiscoveredPeer? resolveKey(String input) {
    final normalized = input.replaceAll('-', '').toUpperCase();
    return keyToPeer[normalized];
  }

  /// Normalise any key format to XXXX-XXXX uppercase.
  static String _normaliseKey(String raw) {
    final clean = raw.replaceAll('-', '').toUpperCase();
    if (clean.length >= 8) {
      return '${clean.substring(0, 4)}-${clean.substring(4, 8)}';
    }
    return clean;
  }

  void stopDiscovery() {
    _beaconTimer?.cancel();
    _beaconTimer = null;
    _socket?.close();
    _socket = null;
  }

  void dispose() {
    stopDiscovery();
    _controller.close();
  }
}
