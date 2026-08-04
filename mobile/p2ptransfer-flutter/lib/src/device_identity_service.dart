import 'dart:io';
import 'package:path_provider/path_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:p2ptransfer/src/rust/api.dart' as rust_api;

/// Generates and manages a persistent device identity key for this device.
///
/// On first launch (or after migration), generates an 8-character uppercase hex string
/// and stores it in the app's documents directory.
class DeviceIdentityService {
  String? _shortId;

  /// Whether the identity has been loaded/generated.
  bool get isInitialized => _shortId != null;

  /// Short 8-character device ID for sharing, formatted as XXXX-XXXX.
  String get deviceId {
    if (_shortId == null) return '------';
    if (_shortId!.length == 8) {
      return '${_shortId!.substring(0, 4)}-${_shortId!.substring(4)}';
    }
    return _shortId!;
  }
  
  /// The raw 8-character uppercase hex string.
  String get rawShortId => _shortId ?? '';

  /// Initialize: load existing key or generate a new one.
  Future<void> initialize() async {
    final dir = await getApplicationDocumentsDirectory();
    final file = File('${dir.path}/.p2p_device_key');

    bool needsReset = false;
    if (await file.exists()) {
      try {
        final content = await file.readAsString();
        final trimmed = content.trim();

        // Check if it's the old 64-char key, or invalid format
        if (trimmed.length != 8 || !RegExp(r'^[0-9A-F]+$').hasMatch(trimmed)) {
          needsReset = true;
        } else {
          _shortId = trimmed;
        }
      } catch (_) {
        // File exists but contains binary/non-UTF-8 data (legacy key).
        // Treat it as corrupt and regenerate.
        needsReset = true;
      }
    } else {
      needsReset = true;
    }

    if (needsReset) {
      await _clearLegacyCache();
      _shortId = rust_api.generateRandomShortId();
      await file.writeAsString(_shortId!);
    }
  }
  
  Future<void> _clearLegacyCache() async {
    // Clear old SharedPreferences
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove('p2p_contacts_v1');
    
    // Clear old identity file if exists
    final dir = await getApplicationDocumentsDirectory();
    final file = File('${dir.path}/.p2p_device_key');
    if (await file.exists()) {
      await file.delete();
    }
    
    // Note: SQLite resume DB on mobile is not yet fully implemented or we can just delete it if it was
    final dbFile = File('${dir.path}/resume.db');
    if (await dbFile.exists()) {
      await dbFile.delete();
    }
  }

  /// Regenerate the device key (old one is replaced).
  Future<void> regenerate() async {
    final dir = await getApplicationDocumentsDirectory();
    final file = File('${dir.path}/.p2p_device_key');

    _shortId = rust_api.generateRandomShortId();
    await file.writeAsString(_shortId!);
  }

  /// Delete the device key file (for testing/reset).
  Future<void> reset() async {
    final dir = await getApplicationDocumentsDirectory();
    final file = File('${dir.path}/.p2p_device_key');
    if (await file.exists()) {
      await file.delete();
    }
    _shortId = null;
  }
}
