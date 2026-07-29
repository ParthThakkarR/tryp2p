import 'dart:convert';
import 'package:shared_preferences/shared_preferences.dart';

/// A saved contact (device key → name mapping).
class Contact {
  final String deviceKey; // XXXX-XXXX short key
  final String name;
  final String? lastIp; // last known IP for LAN resolution
  final int lastPort;
  final DateTime addedAt;

  Contact({
    required this.deviceKey,
    required this.name,
    this.lastIp,
    this.lastPort = 9877,
    DateTime? addedAt,
  }) : addedAt = addedAt ?? DateTime.now();

  Map<String, dynamic> toJson() => {
        'key': deviceKey,
        'name': name,
        'ip': lastIp,
        'port': lastPort,
        'added': addedAt.toIso8601String(),
      };

  factory Contact.fromJson(Map<String, dynamic> j) => Contact(
        deviceKey: j['key'] as String,
        name: j['name'] as String,
        lastIp: j['ip'] as String?,
        lastPort: (j['port'] as num?)?.toInt() ?? 9877,
        addedAt: DateTime.tryParse(j['added'] as String? ?? '') ?? DateTime.now(),
      );

  Contact copyWithIp(String ip, int port) => Contact(
        deviceKey: deviceKey,
        name: name,
        lastIp: ip,
        lastPort: port,
        addedAt: addedAt,
      );
}

/// Persists contacts in SharedPreferences.
class ContactsService {
  ContactsService._();
  static final ContactsService instance = ContactsService._();

  static const _key = 'p2p_contacts_v1';

  List<Contact> _contacts = [];
  List<Contact> get contacts => List.unmodifiable(_contacts);

  Future<void> load() async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_key);
    if (raw == null) return;
    final list = jsonDecode(raw) as List<dynamic>;
    _contacts = list
        .map((e) => Contact.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  Future<void> save() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(
      _key,
      jsonEncode(_contacts.map((c) => c.toJson()).toList()),
    );
  }

  Future<void> addOrUpdate(Contact c) async {
    final idx = _contacts.indexWhere(
        (x) => x.deviceKey.toUpperCase() == c.deviceKey.toUpperCase());
    if (idx >= 0) {
      _contacts[idx] = c;
    } else {
      _contacts.add(c);
    }
    await save();
  }

  Future<void> remove(String deviceKey) async {
    _contacts.removeWhere(
        (c) => c.deviceKey.toUpperCase() == deviceKey.toUpperCase());
    await save();
  }

  Contact? findByKey(String key) {
    final upper = key.toUpperCase().replaceAll('-', '');
    try {
      return _contacts.firstWhere(
        (c) => c.deviceKey.toUpperCase().replaceAll('-', '') == upper,
      );
    } catch (_) {
      return null;
    }
  }

  /// Update last-known IP when a contact is seen on LAN.
  Future<void> updateIp(String deviceKey, String ip, int port) async {
    final idx = _contacts.indexWhere(
        (c) => c.deviceKey.toUpperCase() == deviceKey.toUpperCase());
    if (idx >= 0) {
      _contacts[idx] = _contacts[idx].copyWithIp(ip, port);
      await save();
    }
  }
}
