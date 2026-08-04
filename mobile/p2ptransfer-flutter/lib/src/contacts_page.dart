import 'dart:async';
import 'package:flutter/material.dart';
import 'contacts_service.dart';
import 'peer_discovery_service.dart';
import 'rust/api.dart' as rust_api;

class ContactsPage extends StatefulWidget {
  const ContactsPage({super.key});

  @override
  State<ContactsPage> createState() => _ContactsPageState();
}

class _ContactsPageState extends State<ContactsPage> {
  final _nameController = TextEditingController();
  final _idController = TextEditingController();

  // Online status per contact key (normalised, no dash, uppercase) — true=online
  final Map<String, bool> _onlineStatus = {};
  final PeerDiscoveryService _discovery = PeerDiscoveryService();
  StreamSubscription<List<DiscoveredPeer>>? _discoverySub;
  Timer? _wanPingTimer;

  List<Contact> get _contacts => ContactsService.instance.contacts;

  @override
  void initState() {
    super.initState();
    _startDiscovery();
    _startWanPing();
  }

  void _startDiscovery() async {
    await _discovery.startDiscovery();
    _discoverySub = _discovery.peersStream.listen((peers) {
      if (!mounted) return;
      // Mark LAN peers as online immediately
      final lanKeys = <String>{};
      for (final p in peers) {
        if (p.deviceId != null) {
          lanKeys.add(p.deviceId!.replaceAll('-', '').toUpperCase());
        }
      }
      setState(() {
        for (final key in lanKeys) {
          _onlineStatus[key] = true;
        }
      });
    });
  }

  void _startWanPing() {
    // Run WAN ping immediately, then every 15 seconds
    _runWanPing();
    _wanPingTimer = Timer.periodic(const Duration(seconds: 15), (_) => _runWanPing());
  }

  Future<void> _runWanPing() async {
    for (final c in ContactsService.instance.contacts) {
      final rawKey = c.deviceKey.replaceAll('-', '').toUpperCase();
      try {
        final online = await rust_api.checkPeerOnline(shortId: rawKey);
        if (mounted) {
          setState(() => _onlineStatus[rawKey] = online);
        }
      } catch (_) {}
    }
  }

  bool _isOnline(Contact c) {
    final key = c.deviceKey.replaceAll('-', '').toUpperCase();
    return _onlineStatus[key] ?? false;
  }

  @override
  void dispose() {
    _discoverySub?.cancel();
    _wanPingTimer?.cancel();
    _discovery.dispose();
    _nameController.dispose();
    _idController.dispose();
    super.dispose();
  }

  Future<void> _addContact() async {
    final name = _nameController.text.trim();
    final rawId =
        _idController.text.trim().toUpperCase().replaceAll('-', '');

    if (name.isEmpty ||
        rawId.length != 8 ||
        !RegExp(r'^[0-9A-F]+$').hasMatch(rawId)) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
            content: Text('Enter a valid name and 8-char hex ID (XXXX-XXXX).')),
      );
      return;
    }

    final displayKey = '${rawId.substring(0, 4)}-${rawId.substring(4)}';

    await ContactsService.instance.addOrUpdate(
      Contact(deviceKey: displayKey, name: name),
    );

    if (!mounted) return;
    setState(() {});
    _nameController.clear();
    _idController.clear();
    FocusScope.of(context).unfocus();

    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text('$name added to contacts!'),
        behavior: SnackBarBehavior.floating,
        shape:
            RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
    );
  }

  Future<void> _removeContact(String deviceKey) async {
    await ContactsService.instance.remove(deviceKey);
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Contacts'),
      ),
      body: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // ── Add Contact Form ─────────────────────────────────────────
            Card(
              elevation: 0,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(16),
                side: BorderSide(color: theme.dividerColor, width: 0.5),
              ),
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Add New Contact',
                      style: theme.textTheme.titleSmall
                          ?.copyWith(fontWeight: FontWeight.w600),
                    ),
                    const SizedBox(height: 12),
                    TextField(
                      controller: _nameController,
                      decoration: const InputDecoration(
                        labelText: 'Name',
                        hintText: 'Alice',
                        prefixIcon: Icon(Icons.person_outline),
                      ),
                      textCapitalization: TextCapitalization.words,
                    ),
                    const SizedBox(height: 12),
                    TextField(
                      controller: _idController,
                      decoration: const InputDecoration(
                        labelText: 'Device ID',
                        hintText: 'XXXX-XXXX',
                        prefixIcon: Icon(Icons.key_rounded),
                        helperText:
                            'Ask the other device to share their ID from the home screen.',
                      ),
                      maxLength: 9,
                      keyboardType: TextInputType.visiblePassword,
                      textCapitalization: TextCapitalization.characters,
                      onChanged: (val) {
                        String clean = val
                            .toUpperCase()
                            .replaceAll(RegExp(r'[^0-9A-F]'), '');
                        if (clean.length > 4) {
                          clean =
                              '${clean.substring(0, 4)}-${clean.substring(4)}';
                        }
                        if (_idController.text != clean) {
                          _idController.value = TextEditingValue(
                            text: clean,
                            selection:
                                TextSelection.collapsed(offset: clean.length),
                          );
                        }
                      },
                    ),
                    const SizedBox(height: 8),
                    SizedBox(
                      width: double.infinity,
                      child: FilledButton.icon(
                        onPressed: _addContact,
                        icon: const Icon(Icons.person_add_outlined),
                        label: const Text('Add Contact'),
                        style: FilledButton.styleFrom(
                          padding: const EdgeInsets.symmetric(vertical: 14),
                          shape: RoundedRectangleBorder(
                              borderRadius: BorderRadius.circular(12)),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),

            const SizedBox(height: 20),

            // ── Contact List Header ─────────────────────────────────────
            Row(
              children: [
                Text(
                  'Saved Contacts',
                  style: theme.textTheme.titleSmall
                      ?.copyWith(fontWeight: FontWeight.w600),
                ),
                const SizedBox(width: 8),
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                  decoration: BoxDecoration(
                    color: cs.primaryContainer,
                    borderRadius: BorderRadius.circular(20),
                  ),
                  child: Text(
                    '${_contacts.length}',
                    style: theme.textTheme.labelSmall?.copyWith(
                        color: cs.primary, fontWeight: FontWeight.bold),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 10),

            Expanded(
              child: _contacts.isEmpty
                  ? Center(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(Icons.contacts_outlined,
                              size: 48,
                              color: cs.onSurface.withValues(alpha: 0.2)),
                          const SizedBox(height: 12),
                          Text('No contacts yet.',
                              style: theme.textTheme.bodyMedium?.copyWith(
                                  color: cs.onSurface.withValues(alpha: 0.5))),
                          const SizedBox(height: 4),
                          Text(
                            'Add a contact above to send files quickly.',
                            style: theme.textTheme.bodySmall?.copyWith(
                                color: cs.onSurface.withValues(alpha: 0.35)),
                          ),
                        ],
                      ),
                    )
                  : ListView.separated(
                      itemCount: _contacts.length,
                      separatorBuilder: (_, __) => const SizedBox(height: 6),
                      itemBuilder: (context, index) {
                        final c = _contacts[index];
                        final online = _isOnline(c);
                        return Card(
                          elevation: 0,
                          shape: RoundedRectangleBorder(
                            side: BorderSide(
                                color: theme.dividerColor, width: 0.5),
                            borderRadius: BorderRadius.circular(14),
                          ),
                          child: ListTile(
                            leading: Stack(
                              clipBehavior: Clip.none,
                              children: [
                                CircleAvatar(
                                  backgroundColor: cs.secondaryContainer,
                                  child: Text(
                                    c.name.isNotEmpty
                                        ? c.name[0].toUpperCase()
                                        : '?',
                                    style: TextStyle(
                                        color: cs.secondary,
                                        fontWeight: FontWeight.bold),
                                  ),
                                ),
                                // Online/offline dot
                                Positioned(
                                  right: -2,
                                  bottom: -2,
                                  child: Container(
                                    width: 12,
                                    height: 12,
                                    decoration: BoxDecoration(
                                      color: online
                                          ? Colors.green
                                          : cs.onSurface
                                              .withValues(alpha: 0.25),
                                      shape: BoxShape.circle,
                                      border: Border.all(
                                        color: theme.scaffoldBackgroundColor,
                                        width: 2,
                                      ),
                                    ),
                                  ),
                                ),
                              ],
                            ),
                            title: Row(
                              children: [
                                Text(c.name,
                                    style: const TextStyle(
                                        fontWeight: FontWeight.w600)),
                                const SizedBox(width: 6),
                                Text(
                                  online ? 'Online' : 'Offline',
                                  style: theme.textTheme.labelSmall?.copyWith(
                                    color: online
                                        ? Colors.green
                                        : cs.onSurface.withValues(alpha: 0.4),
                                    fontWeight: FontWeight.w500,
                                  ),
                                ),
                              ],
                            ),
                            subtitle: Text(
                              c.deviceKey,
                              style: theme.textTheme.bodySmall?.copyWith(
                                fontFamily: 'monospace',
                                color: cs.primary.withValues(alpha: 0.8),
                                letterSpacing: 1.5,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                            trailing: IconButton(
                              icon: const Icon(Icons.delete_outline),
                              color: theme.colorScheme.error,
                              onPressed: () => _removeContact(c.deviceKey),
                              tooltip: 'Remove contact',
                            ),
                          ),
                        );
                      },
                    ),
            ),
          ],
        ),
      ),
    );
  }
}
