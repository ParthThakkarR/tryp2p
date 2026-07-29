import 'dart:async';
import 'dart:convert';
import 'dart:io';
import '../main.dart';
import 'settings_page.dart';

class DiscoveredPeer {
  final String deviceName;
  final String address;
  final int port;
  final String? deviceId;
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
  final _peersMap = <String, DiscoveredPeer>{};
  final _controller = StreamController<List<DiscoveredPeer>>.broadcast();

  Stream<List<DiscoveredPeer>> get peersStream => _controller.stream;
  List<DiscoveredPeer> get currentPeers => _peersMap.values.toList();

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
          if (datagram != null) {
            _handleDatagram(datagram);
          }
        }
      });

      _sendBeacon();
    } catch (e) {
      // Socket bind error fallback
    }
  }

  void _sendBeacon() {
    try {
      final beaconData = jsonEncode({
        'device_name': AppSettings.deviceName.isNotEmpty
            ? AppSettings.deviceName
            : 'MobileDevice',
        'p2ptransfer_version': '1.0.0',
        'tcp_port': AppSettings.tcpPort,
        'device_id': deviceIdentity.isInitialized ? deviceIdentity.deviceId : null,
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
      final deviceId = json['device_id'] as String?;
      final senderIp = datagram.address.address;

      final peerKey = '$senderIp:$port';
      _peersMap[peerKey] = DiscoveredPeer(
        deviceName: deviceName,
        address: senderIp,
        port: port,
        deviceId: deviceId,
        lastSeen: DateTime.now(),
      );

      _controller.add(currentPeers);
    } catch (_) {}
  }

  void stopDiscovery() {
    _socket?.close();
    _socket = null;
  }

  void dispose() {
    stopDiscovery();
    _controller.close();
  }
}
