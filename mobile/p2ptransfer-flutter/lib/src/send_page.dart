import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:file_picker/file_picker.dart';
import '../main.dart';
import 'peer_discovery_service.dart';
import 'settings_page.dart';
import 'transfer_service.dart';
import 'contacts_service.dart';
import 'widgets/device_key_card.dart';

class SendPage extends StatefulWidget {
  const SendPage({super.key});

  @override
  State<SendPage> createState() => _SendPageState();
}

class _SendPageState extends State<SendPage> {
  // File
  String? _filePath;
  String? _fileName;
  int? _fileSize;

  // Target selection
  final _keyController = TextEditingController();
  String? _selectedPeerKey;   // normalised key (no dash, uppercase)
  String? _selectedPeerName;
  String? _resolvedIp;
  int _resolvedPort = 9877;

  // Discovery
  final PeerDiscoveryService _discoveryService = PeerDiscoveryService();
  List<DiscoveredPeer> _discoveredPeers = [];
  bool _isScanning = false;
  bool _showManualKey = false;

  // Transfer state
  bool _sending = false;
  double _sendProgress = 0;
  String _sendStatus = '';
  bool _sendDone = false;
  bool _sendError = false;
  StreamSubscription<TransferProgress>? _progressSub;

  // After success: offer to save contact
  bool _offerSaveContact = false;
  final _contactNameController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _startPeerScan();
  }

  void _startPeerScan() async {
    setState(() => _isScanning = true);
    await _discoveryService.startDiscovery();
    _discoveryService.peersStream.listen((peers) {
      if (mounted) setState(() { _discoveredPeers = peers; _isScanning = false; });
    });
    Future.delayed(const Duration(seconds: 5), () {
      if (mounted) setState(() => _isScanning = false);
    });
  }

  @override
  void dispose() {
    _discoveryService.dispose();
    _keyController.dispose();
    _contactNameController.dispose();
    _progressSub?.cancel();
    super.dispose();
  }

  // ── File picking ─────────────────────────────────────────────────────────
  Future<void> _pickFile() async {
    final result = await FilePicker.platform.pickFiles();
    if (result != null) {
      final file = result.files.single;
      setState(() { _filePath = file.path; _fileName = file.name; _fileSize = file.size; });
    }
  }

  // ── Key / target resolution ───────────────────────────────────────────────
  String get _rawKey =>
      (_selectedPeerKey ?? _keyController.text.replaceAll('-', '').toUpperCase()).toUpperCase();

  bool get _hasValidKey => _rawKey.length == 8;

  String _formatDisplayKey(String raw) {
    final c = raw.replaceAll('-', '').toUpperCase();
    if (c.length >= 8) return '${c.substring(0, 4)}-${c.substring(4, 8)}';
    return c;
  }

  /// Try to find IP: LAN discovery first, then contacts service.
  bool _resolveTarget() {
    final raw = _rawKey;
    // 1. LAN discovery
    final peer = _discoveryService.resolveKey(raw);
    if (peer != null) {
      _resolvedIp = peer.address;
      _resolvedPort = peer.port;
      _selectedPeerName ??= peer.deviceName;
      return true;
    }
    // 2. Saved contacts (last-known IP)
    final contact = ContactsService.instance.findByKey(raw);
    if (contact != null && contact.lastIp != null) {
      _resolvedIp = contact.lastIp;
      _resolvedPort = contact.lastPort;
      _selectedPeerName ??= contact.name;
      return true;
    }
    return false;
  }

  // ── Send ─────────────────────────────────────────────────────────────────
  Future<void> _sendTransfer() async {
    if (_filePath == null || !_hasValidKey) return;

    if (!_resolveTarget()) {
      _showSnack(
        'Device not found on LAN and not in contacts.\n'
        'Make sure both devices are on the same Wi-Fi and the receiver is open.',
        isError: true,
      );
      return;
    }

    setState(() {
      _sending = true;
      _sendProgress = 0;
      _sendStatus = 'Connecting…';
      _sendDone = false;
      _sendError = false;
      _offerSaveContact = false;
    });

    _progressSub?.cancel();
    _progressSub = TransferService.instance.sendFile(
      peerIp: _resolvedIp!,
      peerPort: _resolvedPort,
      filePath: _filePath!,
    ).listen(
      (p) {
        if (!mounted) return;
        setState(() {
          _sendProgress = p.fraction;
          if (p.isDone) {
            _sendStatus = 'Complete ✓';
            _sendDone = true;
            _offerSaveContact = ContactsService.instance.findByKey(_rawKey) == null;
          } else if (p.isError) {
            _sendStatus = 'Error: ${p.errorMessage}';
            _sendError = true;
          } else {
            _sendStatus = '${(_sendProgress * 100).toInt()}%  •  ${p.speedLabel}';
          }
        });
      },
      onError: (e) {
        if (mounted) setState(() { _sendError = true; _sendStatus = 'Error: $e'; });
      },
    );
  }

  void _saveContact() async {
    final name = _contactNameController.text.trim();
    if (name.isEmpty) return;
    await ContactsService.instance.addOrUpdate(Contact(
      deviceKey: _formatDisplayKey(_rawKey),
      name: name,
      lastIp: _resolvedIp,
      lastPort: _resolvedPort,
    ));
    setState(() => _offerSaveContact = false);
    _showSnack('$name saved to contacts!');
  }

  void _resetSend() {
    _progressSub?.cancel();
    setState(() {
      _sending = false;
      _sendProgress = 0;
      _sendStatus = '';
      _sendDone = false;
      _sendError = false;
      _offerSaveContact = false;
    });
  }

  void _showSnack(String msg, {bool isError = false}) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(
      content: Text(msg),
      backgroundColor: isError ? Theme.of(context).colorScheme.error : null,
      behavior: SnackBarBehavior.floating,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
    ));
  }

  String _formatFileSize(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
    if (bytes < 1024 * 1024 * 1024) return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
  }

  // ── Build ─────────────────────────────────────────────────────────────────
  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Send File'),
        actions: [
          IconButton(
            icon: const Icon(Icons.settings_outlined),
            onPressed: () => Navigator.pushNamed(context, '/settings').then((_) => setState(() {})),
            tooltip: 'Settings',
          ),
        ],
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // My key
            DeviceKeyCard(
              deviceId: deviceIdentity.deviceId,
              fullKeyHex: deviceIdentity.fullKeyHex,
              compact: true,
            ),
            const SizedBox(height: 20),

            // ── File selection ──────────────────────────────────────────
            if (!_sending) ...[
              Text('Select File',
                  style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w600)),
              const SizedBox(height: 8),
              _FileCard(
                filePath: _filePath,
                fileName: _fileName,
                fileSize: _fileSize,
                formatSize: _formatFileSize,
                onPick: _pickFile,
                onClear: () => setState(() { _filePath = null; _fileName = null; _fileSize = null; }),
              ),
              const SizedBox(height: 24),

              // ── Target device ─────────────────────────────────────────
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text('Target Device',
                      style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w600)),
                  if (_isScanning)
                    const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2))
                  else
                    TextButton.icon(
                      onPressed: () { _discoveredPeers.clear(); _startPeerScan(); },
                      icon: const Icon(Icons.refresh, size: 16),
                      label: const Text('Scan'),
                    ),
                ],
              ),
              const SizedBox(height: 8),

              // Saved contacts (quick select)
              if (ContactsService.instance.contacts.isNotEmpty) ...[
                Text('Saved Contacts',
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: cs.onSurface.withValues(alpha: 0.5),
                    )),
                const SizedBox(height: 6),
                ...ContactsService.instance.contacts.map(
                  (c) => _buildContactTile(c, theme, cs),
                ),
                const SizedBox(height: 12),
              ],

              // LAN discovered peers
              if (_discoveredPeers.isNotEmpty) ...[
                Text('Nearby on LAN',
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: cs.onSurface.withValues(alpha: 0.5),
                    )),
                const SizedBox(height: 6),
                ..._discoveredPeers.map((p) => _buildPeerTile(p, theme, cs)),
              ],

              if (_discoveredPeers.isEmpty && ContactsService.instance.contacts.isEmpty && !_isScanning)
                _EmptyLanCard(theme: theme, cs: cs),

              // Manual key entry
              const SizedBox(height: 8),
              TextButton.icon(
                onPressed: () => setState(() => _showManualKey = !_showManualKey),
                icon: Icon(_showManualKey ? Icons.expand_less : Icons.expand_more, size: 18),
                label: Text(_showManualKey ? 'Hide key entry' : 'Enter device key manually'),
              ),

              if (_showManualKey) ...[
                const SizedBox(height: 8),
                _KeyEntryCard(
                  controller: _keyController,
                  cs: cs,
                  theme: theme,
                  onChanged: (val) {
                    setState(() {
                      _selectedPeerKey = null;
                      _selectedPeerName = null;
                    });
                  },
                ),
                if (_keyController.text.isNotEmpty && !_hasValidKey)
                  Padding(
                    padding: const EdgeInsets.only(left: 16, top: 4),
                    child: Text('Key must be 8 hex characters (e.g. A3F8-K2D1)',
                        style: theme.textTheme.bodySmall?.copyWith(color: cs.error)),
                  ),
              ],

              // Active target banner
              if (_hasValidKey) ...[
                const SizedBox(height: 12),
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
                  decoration: BoxDecoration(
                    color: cs.primaryContainer.withValues(alpha: 0.4),
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Row(
                    children: [
                      Icon(Icons.check_circle_outline, size: 18, color: cs.primary),
                      const SizedBox(width: 8),
                      Text('Sending to: ',
                          style: theme.textTheme.bodySmall?.copyWith(
                              color: cs.onSurface.withValues(alpha: 0.6))),
                      Text(
                        _selectedPeerName != null
                            ? '$_selectedPeerName  •  ${_formatDisplayKey(_rawKey)}'
                            : _formatDisplayKey(_rawKey),
                        style: theme.textTheme.bodySmall?.copyWith(
                          fontFamily: 'monospace',
                          fontWeight: FontWeight.w700,
                          color: cs.primary,
                          letterSpacing: 1.5,
                        ),
                      ),
                    ],
                  ),
                ),
              ],

              const SizedBox(height: 24),

              // Compression info
              Row(
                children: [
                  Icon(Icons.speed, size: 16, color: cs.onSurface.withValues(alpha: 0.4)),
                  const SizedBox(width: 6),
                  Text('Compression: Level ${AppSettings.compressionLevel}',
                      style: theme.textTheme.bodySmall?.copyWith(
                          color: cs.onSurface.withValues(alpha: 0.5))),
                ],
              ),
              const SizedBox(height: 16),

              // Send button
              SizedBox(
                width: double.infinity,
                child: FilledButton.icon(
                  onPressed: (_filePath != null && _hasValidKey) ? _sendTransfer : null,
                  icon: const Icon(Icons.send_rounded),
                  label: const Text('Send File',
                      style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600)),
                  style: FilledButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 16),
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
                  ),
                ),
              ),
            ],

            // ── Transfer in progress / done ──────────────────────────────
            if (_sending) ...[
              _TransferProgressCard(
                fileName: _fileName ?? '',
                fileSize: _fileSize ?? 0,
                progress: _sendProgress,
                statusLabel: _sendStatus,
                isDone: _sendDone,
                isError: _sendError,
                formatSize: _formatFileSize,
                onReset: _resetSend,
                theme: theme,
                cs: cs,
              ),

              // Save contact offer
              if (_offerSaveContact && _sendDone) ...[
                const SizedBox(height: 16),
                Card(
                  elevation: 0,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(16),
                    side: BorderSide(color: cs.primary.withValues(alpha: 0.3), width: 1),
                  ),
                  child: Padding(
                    padding: const EdgeInsets.all(16),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text('Save as contact?',
                            style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w600)),
                        const SizedBox(height: 8),
                        Row(
                          children: [
                            Expanded(
                              child: TextField(
                                controller: _contactNameController,
                                decoration: const InputDecoration(
                                  hintText: 'Contact name',
                                  isDense: true,
                                ),
                              ),
                            ),
                            const SizedBox(width: 8),
                            FilledButton(
                              onPressed: _saveContact,
                              child: const Text('Save'),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ],
          ],
        ),
      ),
    );
  }

  // ── Peer / contact tiles ──────────────────────────────────────────────────
  Widget _buildContactTile(Contact c, ThemeData theme, ColorScheme cs) {
    final key = c.deviceKey.replaceAll('-', '').toUpperCase();
    final isSelected = _selectedPeerKey == key;
    return Card(
      elevation: 0,
      margin: const EdgeInsets.only(bottom: 6),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
        side: BorderSide(
          color: isSelected ? cs.primary : theme.dividerColor,
          width: isSelected ? 1.5 : 0.5,
        ),
      ),
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: () => setState(() {
          _selectedPeerKey = key;
          _selectedPeerName = c.name;
          _resolvedIp = c.lastIp;
          _resolvedPort = c.lastPort;
          _keyController.clear();
          _showManualKey = false;
        }),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
          child: Row(
            children: [
              CircleAvatar(
                radius: 18,
                backgroundColor: cs.secondaryContainer,
                child: Icon(Icons.person, color: cs.secondary, size: 18),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(c.name,
                        style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600)),
                    Text(c.deviceKey,
                        style: theme.textTheme.bodySmall?.copyWith(
                          fontFamily: 'monospace',
                          color: cs.primary.withValues(alpha: 0.8),
                          letterSpacing: 1.5,
                          fontWeight: FontWeight.w600,
                        )),
                  ],
                ),
              ),
              if (isSelected)
                Icon(Icons.check_circle, color: cs.primary, size: 22)
              else
                Icon(Icons.chevron_right, color: cs.onSurface.withValues(alpha: 0.3)),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildPeerTile(DiscoveredPeer peer, ThemeData theme, ColorScheme cs) {
    final peerKey = peer.deviceId?.replaceAll('-', '').toUpperCase() ?? peer.fullAddress;
    final displayKey = peer.deviceId ?? peer.fullAddress;
    final isSelected = _selectedPeerKey == peerKey;

    return Card(
      elevation: 0,
      margin: const EdgeInsets.only(bottom: 6),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
        side: BorderSide(
          color: isSelected ? cs.primary : theme.dividerColor,
          width: isSelected ? 1.5 : 0.5,
        ),
      ),
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: () => setState(() {
          _selectedPeerKey = peerKey;
          _selectedPeerName = peer.deviceName;
          _resolvedIp = peer.address;
          _resolvedPort = peer.port;
          _keyController.clear();
          _showManualKey = false;
        }),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
          child: Row(
            children: [
              CircleAvatar(
                radius: 18,
                backgroundColor: cs.primaryContainer,
                child: Icon(
                  peer.deviceName.toLowerCase().contains('phone') ||
                          peer.deviceName.toLowerCase().contains('mobile')
                      ? Icons.smartphone
                      : Icons.laptop,
                  color: cs.primary,
                  size: 18,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(peer.deviceName,
                        style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600)),
                    Text(displayKey,
                        style: theme.textTheme.bodySmall?.copyWith(
                          fontFamily: 'monospace',
                          color: cs.primary.withValues(alpha: 0.8),
                          letterSpacing: 1.5,
                          fontWeight: FontWeight.w600,
                        )),
                  ],
                ),
              ),
              if (isSelected)
                Icon(Icons.check_circle, color: cs.primary, size: 22)
              else
                Icon(Icons.chevron_right, color: cs.onSurface.withValues(alpha: 0.3)),
            ],
          ),
        ),
      ),
    );
  }
}

// ── Sub-widgets ───────────────────────────────────────────────────────────────

class _FileCard extends StatelessWidget {
  final String? filePath, fileName;
  final int? fileSize;
  final String Function(int) formatSize;
  final VoidCallback onPick, onClear;
  const _FileCard({
    required this.filePath,
    required this.fileName,
    required this.fileSize,
    required this.formatSize,
    required this.onPick,
    required this.onClear,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    return Card(
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: theme.dividerColor, width: 0.5),
      ),
      child: InkWell(
        borderRadius: BorderRadius.circular(16),
        onTap: onPick,
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Row(
            children: [
              Container(
                padding: const EdgeInsets.all(10),
                decoration: BoxDecoration(
                  color: cs.primaryContainer,
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Icon(
                  filePath != null ? Icons.insert_drive_file : Icons.add,
                  color: cs.primary,
                  size: 24,
                ),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      fileName ?? 'Tap to browse files',
                      style: theme.textTheme.bodyMedium
                          ?.copyWith(fontWeight: FontWeight.w500),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    if (fileSize != null)
                      Text(
                        formatSize(fileSize!),
                        style: theme.textTheme.bodySmall?.copyWith(
                            color: cs.onSurface.withValues(alpha: 0.5)),
                      ),
                  ],
                ),
              ),
              if (filePath != null)
                IconButton(
                  icon: const Icon(Icons.close, size: 18),
                  onPressed: onClear,
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _KeyEntryCard extends StatelessWidget {
  final TextEditingController controller;
  final ColorScheme cs;
  final ThemeData theme;
  final ValueChanged<String> onChanged;
  const _KeyEntryCard(
      {required this.controller, required this.cs, required this.theme, required this.onChanged});

  String _format(String raw) {
    final c = raw.replaceAll('-', '').toUpperCase().replaceAll(RegExp(r'[^A-F0-9]'), '');
    final clamped = c.length > 8 ? c.substring(0, 8) : c;
    if (clamped.length <= 4) return clamped;
    return '${clamped.substring(0, 4)}-${clamped.substring(4)}';
  }

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: theme.dividerColor, width: 0.5),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        child: Row(
          children: [
            Container(
              padding: const EdgeInsets.all(8),
              decoration: BoxDecoration(
                color: cs.secondaryContainer,
                borderRadius: BorderRadius.circular(10),
              ),
              child: Icon(Icons.key_rounded, color: cs.secondary, size: 20),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: TextField(
                controller: controller,
                onChanged: (val) {
                  final formatted = _format(val);
                  if (formatted != val) {
                    controller.value = TextEditingValue(
                      text: formatted,
                      selection:
                          TextSelection.collapsed(offset: formatted.length),
                    );
                  }
                  onChanged(formatted);
                },
                decoration: InputDecoration(
                  labelText: "Receiver's Device Key",
                  hintText: 'A3F8-K2D1',
                  border: InputBorder.none,
                  isDense: true,
                  contentPadding: EdgeInsets.zero,
                  suffixIcon: controller.text.isNotEmpty
                      ? IconButton(
                          icon: const Icon(Icons.close, size: 18),
                          onPressed: () {
                            controller.clear();
                            onChanged('');
                          },
                          padding: EdgeInsets.zero,
                          constraints: const BoxConstraints(),
                        )
                      : null,
                ),
                style: theme.textTheme.bodyLarge?.copyWith(
                  fontFamily: 'monospace',
                  letterSpacing: 2,
                  fontWeight: FontWeight.w600,
                ),
                inputFormatters: [
                  FilteringTextInputFormatter.allow(RegExp(r'[A-Fa-f0-9\-]')),
                  LengthLimitingTextInputFormatter(9),
                ],
                textCapitalization: TextCapitalization.characters,
                keyboardType: TextInputType.visiblePassword,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _EmptyLanCard extends StatelessWidget {
  final ThemeData theme;
  final ColorScheme cs;
  const _EmptyLanCard({required this.theme, required this.cs});

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: theme.dividerColor, width: 0.5),
      ),
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Center(
          child: Column(
            children: [
              Icon(Icons.wifi_find, size: 40, color: cs.onSurface.withValues(alpha: 0.2)),
              const SizedBox(height: 8),
              Text('No nearby devices found',
                  style: theme.textTheme.bodyMedium?.copyWith(
                      color: cs.onSurface.withValues(alpha: 0.5))),
              const SizedBox(height: 4),
              Text(
                'Ensure both devices are on the same Wi-Fi\nor enter the receiver\'s key manually below.',
                textAlign: TextAlign.center,
                style: theme.textTheme.bodySmall?.copyWith(
                    color: cs.onSurface.withValues(alpha: 0.35)),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _TransferProgressCard extends StatelessWidget {
  final String fileName;
  final int fileSize;
  final double progress;
  final String statusLabel;
  final bool isDone, isError;
  final String Function(int) formatSize;
  final VoidCallback onReset;
  final ThemeData theme;
  final ColorScheme cs;

  const _TransferProgressCard({
    required this.fileName,
    required this.fileSize,
    required this.progress,
    required this.statusLabel,
    required this.isDone,
    required this.isError,
    required this.formatSize,
    required this.onReset,
    required this.theme,
    required this.cs,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(20),
        side: BorderSide(
          color: isDone
              ? Colors.green.withValues(alpha: 0.4)
              : isError
                  ? cs.error.withValues(alpha: 0.4)
                  : theme.dividerColor,
          width: isDone || isError ? 1.5 : 0.5,
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(
                  isDone ? Icons.check_circle : isError ? Icons.error : Icons.upload_rounded,
                  color: isDone ? Colors.green : isError ? cs.error : cs.primary,
                  size: 28,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        isDone ? 'Transfer Complete' : isError ? 'Transfer Failed' : 'Sending…',
                        style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
                      ),
                      Text(
                        '$fileName  •  ${fileSize > 0 ? formatSize(fileSize) : ""}',
                        style: theme.textTheme.bodySmall?.copyWith(
                            color: cs.onSurface.withValues(alpha: 0.5)),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),
            LinearProgressIndicator(
              value: isDone ? 1.0 : isError ? 0 : (progress > 0 ? progress : null),
              minHeight: 8,
              borderRadius: BorderRadius.circular(4),
              color: isDone ? Colors.green : isError ? cs.error : null,
            ),
            const SizedBox(height: 8),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text(statusLabel,
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: isDone ? Colors.green : isError ? cs.error : null,
                      fontWeight: FontWeight.w600,
                    )),
                if (isDone || isError)
                  TextButton(
                    onPressed: onReset,
                    child: const Text('Send Another'),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
