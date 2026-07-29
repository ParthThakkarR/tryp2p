import 'dart:math';
import 'dart:io';
import 'package:path_provider/path_provider.dart';

/// Generates and manages a persistent device identity key for this device.
///
/// On first launch, generates a 32-byte random seed and stores it in the app's
/// documents directory. Derives a short human-readable device ID (8 characters,
/// hex format with a dash) that is easy to share (e.g. "A3F8-K2D1").
class DeviceIdentityService {
  static const int _seedLength = 32;

  List<int>? _keySeed;
  String? _shortId;
  String? _fullKeyHex;

  /// Whether the identity has been loaded/generated.
  bool get isInitialized => _keySeed != null;

  /// Short 8-character device ID for sharing (e.g. "A3F8-K2D1").
  String get deviceId => _shortId ?? '------';

  /// Full 32-byte seed as 64-char hex string.
  String get fullKeyHex => _fullKeyHex ?? '';

  /// Initialize: load existing key or generate a new one.
  Future<void> initialize() async {
    final dir = await getApplicationDocumentsDirectory();
    final file = File('${dir.path}/.p2p_device_key');

    if (await file.exists()) {
      _keySeed = await file.readAsBytes();
      if (_keySeed!.length != _seedLength) {
        _keySeed = _generateSeed();
        await file.writeAsBytes(_keySeed!);
      }
    } else {
      _keySeed = _generateSeed();
      await file.writeAsBytes(_keySeed!);
    }

    _deriveIdentifiers();
  }

  /// Regenerate the device key (old one is replaced).
  Future<void> regenerate() async {
    final dir = await getApplicationDocumentsDirectory();
    final file = File('${dir.path}/.p2p_device_key');

    _keySeed = _generateSeed();
    await file.writeAsBytes(_keySeed!);
    _deriveIdentifiers();
  }

  /// Delete the device key file (for testing/reset).
  Future<void> reset() async {
    final dir = await getApplicationDocumentsDirectory();
    final file = File('${dir.path}/.p2p_device_key');
    if (await file.exists()) {
      await file.delete();
    }
    _keySeed = null;
    _shortId = null;
    _fullKeyHex = null;
  }

  /// Get the raw seed bytes.
  List<int>? get rawSeed => _keySeed;

  List<int> _generateSeed() {
    final random = Random.secure();
    return List<int>.generate(_seedLength, (_) => random.nextInt(256));
  }

  void _deriveIdentifiers() {
    if (_keySeed == null) return;

    // Full key: hex of all 32 bytes
    _fullKeyHex = _keySeed!.map((b) => b.toRadixString(16).padLeft(2, '0')).join();

    // Short ID: mix seed bytes with XOR + rotate to get a deterministic
    // pseudo-hash, then take first 4 bytes as 8 hex chars.
    final mixed = List<int>.generate(32, (i) {
      int v = _keySeed![i];
      v ^= _keySeed![(i + 3) % 32];
      v ^= _keySeed![(i + 7) % 32];
      v = ((v << 3) | (v >> 5)) & 0xFF;
      v ^= 0x5A;
      return v;
    });

    final shortBytes = mixed.take(4).toList();
    final hex = shortBytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
    _shortId = '${hex.substring(0, 4)}-${hex.substring(4)}';
  }
}
