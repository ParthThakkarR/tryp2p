import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:file_picker/file_picker.dart';
import '../main.dart';
import 'peer_discovery_service.dart';
import 'settings_page.dart';
import 'widgets/device_key_card.dart';

class SendPage extends StatefulWidget {
  const SendPage({super.key});

  @override
  State<SendPage> createState() => _SendPageState();
}

class _SendPageState extends State<SendPage> {
  String? _filePath;
  String? _fileName;
  int? _fileSize;

  // Key-based targeting (replaces IP:Port)
  final _keyController = TextEditingController();
  String? _selectedPeerKey;   // key of a discovered peer tapped from the list
  String? _selectedPeerName;

  final PeerDiscoveryService _discoveryService = PeerDiscoveryService();
  List<DiscoveredPeer> _discoveredPeers = [];
  bool _isScanning = false;
  bool _showManualKey = false;

  @override
  void initState() {
    super.initState();
    _startPeerScan();
  }

  void _startPeerScan() async {
    setState(() => _isScanning = true);
    await _discoveryService.startDiscovery();
    _discoveryService.peersStream.listen((peers) {
      if (mounted) {
        setState(() {
          _discoveredPeers = peers;
          _isScanning = false;
        });
      }
    });

    Future.delayed(const Duration(seconds: 5), () {
      if (mounted) setState(() => _isScanning = false);
    });
  }

  @override
  void dispose() {
    _discoveryService.dispose();
    _keyController.dispose();
    super.dispose();
  }

  Future<void> _pickFile() async {
    final result = await FilePicker.platform.pickFiles();
    if (result != null) {
      final file = result.files.single;
      setState(() {
        _filePath = file.path;
        _fileName = file.name;
        _fileSize = file.size;
      });
    }
  }

  /// The active target key — either tapped from discovery list or typed manually.
  String get _activeKey {
    if (_selectedPeerKey != null) return _selectedPeerKey!;
    return _keyController.text.trim().replaceAll('-', '').toUpperCase();
  }

  bool get _hasValidKey {
    final raw = _activeKey.replaceAll('-', '');
    return raw.length == 8; // device ID is always 8 hex chars (XXXXXXXX)
  }

  void _sendTransfer() {
    final key = _activeKey;
    final targetLabel = _selectedPeerName != null
        ? '$_selectedPeerName ($key)'
        : key;
    final compression = AppSettings.compressionLevel;

    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
            'Sending to $targetLabel  •  Compression Level $compression'),
        backgroundColor: Colors.blue[800],
        behavior: SnackBarBehavior.floating,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
    );
  }

  String _formatFileSize(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
    if (bytes < 1024 * 1024 * 1024) {
      return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
  }

  /// Format raw 8-char hex as XXXX-XXXX while typing
  String _formatKey(String raw) {
    final cleaned = raw.replaceAll('-', '').toUpperCase();
    if (cleaned.length <= 4) return cleaned;
    return '${cleaned.substring(0, 4)}-${cleaned.substring(4)}';
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Send File'),
        actions: [
          IconButton(
            icon: const Icon(Icons.settings_outlined),
            onPressed: () =>
                Navigator.pushNamed(context, '/settings')
                    .then((_) => setState(() {})),
            tooltip: 'Settings',
          ),
        ],
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // My device key (compact)
            DeviceKeyCard(
              deviceId: deviceIdentity.deviceId,
              fullKeyHex: deviceIdentity.fullKeyHex,
              compact: true,
            ),
            const SizedBox(height: 20),

            // ── File Selection ──────────────────────────────────────────
            Text('Select File',
                style: theme.textTheme.titleSmall
                    ?.copyWith(fontWeight: FontWeight.w600)),
            const SizedBox(height: 8),
            Card(
              elevation: 0,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(16),
                side: BorderSide(color: theme.dividerColor, width: 0.5),
              ),
              child: InkWell(
                borderRadius: BorderRadius.circular(16),
                onTap: _pickFile,
                child: Padding(
                  padding: const EdgeInsets.all(16),
                  child: Row(
                    children: [
                      Container(
                        padding: const EdgeInsets.all(10),
                        decoration: BoxDecoration(
                          color: colorScheme.primaryContainer,
                          borderRadius: BorderRadius.circular(12),
                        ),
                        child: Icon(
                          _filePath != null
                              ? Icons.insert_drive_file
                              : Icons.add,
                          color: colorScheme.primary,
                          size: 24,
                        ),
                      ),
                      const SizedBox(width: 14),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              _fileName ?? 'Tap to browse files',
                              style: theme.textTheme.bodyMedium
                                  ?.copyWith(fontWeight: FontWeight.w500),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                            if (_fileSize != null)
                              Text(
                                _formatFileSize(_fileSize!),
                                style: theme.textTheme.bodySmall?.copyWith(
                                  color: colorScheme.onSurface
                                      .withValues(alpha: 0.5),
                                ),
                              ),
                          ],
                        ),
                      ),
                      if (_filePath != null)
                        IconButton(
                          icon: const Icon(Icons.close, size: 18),
                          onPressed: () => setState(() {
                            _filePath = null;
                            _fileName = null;
                            _fileSize = null;
                          }),
                          padding: EdgeInsets.zero,
                          constraints: const BoxConstraints(),
                        ),
                    ],
                  ),
                ),
              ),
            ),

            const SizedBox(height: 24),

            // ── Target Device ───────────────────────────────────────────
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text('Target Device',
                    style: theme.textTheme.titleSmall
                        ?.copyWith(fontWeight: FontWeight.w600)),
                if (_isScanning)
                  const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                else
                  TextButton.icon(
                    onPressed: _startPeerScan,
                    icon: const Icon(Icons.refresh, size: 16),
                    label: const Text('Scan'),
                  ),
              ],
            ),
            const SizedBox(height: 8),

            // ── Discovered peers (show device key, not IP) ──────────────
            if (_discoveredPeers.isNotEmpty)
              ..._discoveredPeers
                  .map((peer) => _buildPeerTile(peer, theme, colorScheme)),

            if (_discoveredPeers.isEmpty && !_isScanning)
              Card(
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
                        Icon(Icons.wifi_find,
                            size: 40,
                            color: colorScheme.onSurface.withValues(alpha: 0.2)),
                        const SizedBox(height: 8),
                        Text('No nearby devices found',
                            style: theme.textTheme.bodyMedium?.copyWith(
                              color: colorScheme.onSurface.withValues(alpha: 0.5),
                            )),
                        const SizedBox(height: 4),
                        Text(
                          'Ensure both devices are on the same Wi-Fi,\nor enter the receiver\'s key manually below.',
                          textAlign: TextAlign.center,
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: colorScheme.onSurface.withValues(alpha: 0.35),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),

            // ── Manual key entry ────────────────────────────────────────
            const SizedBox(height: 8),
            TextButton.icon(
              onPressed: () =>
                  setState(() => _showManualKey = !_showManualKey),
              icon: Icon(
                _showManualKey ? Icons.expand_less : Icons.expand_more,
                size: 18,
              ),
              label: Text(
                _showManualKey
                    ? 'Hide key entry'
                    : 'Enter device key manually',
              ),
            ),

            if (_showManualKey) ...[
              const SizedBox(height: 8),
              // Key entry card with styled input
              Card(
                elevation: 0,
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(16),
                  side: BorderSide(
                    color: _hasValidKey && _selectedPeerKey == null
                        ? colorScheme.primary
                        : theme.dividerColor,
                    width: _hasValidKey && _selectedPeerKey == null ? 1.5 : 0.5,
                  ),
                ),
                child: Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                  child: Row(
                    children: [
                      Container(
                        padding: const EdgeInsets.all(8),
                        decoration: BoxDecoration(
                          color: colorScheme.secondaryContainer,
                          borderRadius: BorderRadius.circular(10),
                        ),
                        child: Icon(Icons.key_rounded,
                            color: colorScheme.secondary, size: 20),
                      ),
                      const SizedBox(width: 12),
                      Expanded(
                        child: TextField(
                          controller: _keyController,
                          onChanged: (val) {
                            // Auto-format as XXXX-XXXX
                            final raw = val
                                .replaceAll('-', '')
                                .toUpperCase()
                                .replaceAll(RegExp(r'[^A-F0-9]'), '');
                            final clamped =
                                raw.length > 8 ? raw.substring(0, 8) : raw;
                            final formatted = _formatKey(clamped);
                            if (formatted != val) {
                              _keyController.value = TextEditingValue(
                                text: formatted,
                                selection: TextSelection.collapsed(
                                    offset: formatted.length),
                              );
                            }
                            setState(() {
                              // Clear selected peer when typing manually
                              if (_selectedPeerKey != null) {
                                _selectedPeerKey = null;
                                _selectedPeerName = null;
                              }
                            });
                          },
                          decoration: InputDecoration(
                            labelText: 'Receiver\'s Device Key',
                            hintText: 'A3F8-K2D1',
                            border: InputBorder.none,
                            isDense: true,
                            contentPadding: EdgeInsets.zero,
                            suffixIcon: _keyController.text.isNotEmpty
                                ? IconButton(
                                    icon: const Icon(Icons.close, size: 18),
                                    onPressed: () {
                                      _keyController.clear();
                                      setState(() {});
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
                            FilteringTextInputFormatter.allow(
                                RegExp(r'[A-Fa-f0-9\-]')),
                            LengthLimitingTextInputFormatter(9), // 8 chars + dash
                          ],
                          textCapitalization: TextCapitalization.characters,
                          keyboardType: TextInputType.visiblePassword,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
              if (_keyController.text.isNotEmpty && !_hasValidKey)
                Padding(
                  padding: const EdgeInsets.only(left: 16, top: 4),
                  child: Text(
                    'Key must be 8 hex characters (e.g. A3F8-K2D1)',
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: colorScheme.error,
                    ),
                  ),
                ),
            ],

            // ── Active target indicator ────────────────────────────────
            if (_hasValidKey) ...[
              const SizedBox(height: 12),
              Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
                decoration: BoxDecoration(
                  color: colorScheme.primaryContainer.withValues(alpha: 0.4),
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Row(
                  children: [
                    Icon(Icons.check_circle_outline,
                        size: 18, color: colorScheme.primary),
                    const SizedBox(width: 8),
                    Text('Sending to: ',
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: colorScheme.onSurface.withValues(alpha: 0.6),
                        )),
                    Text(
                      _selectedPeerName != null
                          ? '$_selectedPeerName  •  $_selectedPeerKey'
                          : _formatKey(_activeKey),
                      style: theme.textTheme.bodySmall?.copyWith(
                        fontFamily: 'monospace',
                        fontWeight: FontWeight.w700,
                        color: colorScheme.primary,
                        letterSpacing: 1.5,
                      ),
                    ),
                  ],
                ),
              ),
            ],

            const SizedBox(height: 24),

            // ── Compression info ────────────────────────────────────────
            Row(
              children: [
                Icon(Icons.speed,
                    size: 16,
                    color: colorScheme.onSurface.withValues(alpha: 0.4)),
                const SizedBox(width: 6),
                Text(
                  'Compression: Level ${AppSettings.compressionLevel}',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: colorScheme.onSurface.withValues(alpha: 0.5),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),

            // ── Send Button ─────────────────────────────────────────────
            SizedBox(
              width: double.infinity,
              child: FilledButton.icon(
                onPressed: (_filePath != null && _hasValidKey)
                    ? _sendTransfer
                    : null,
                icon: const Icon(Icons.send_rounded),
                label: const Text('Send File',
                    style: TextStyle(
                        fontSize: 16, fontWeight: FontWeight.w600)),
                style: FilledButton.styleFrom(
                  padding: const EdgeInsets.symmetric(vertical: 16),
                  shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(14)),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildPeerTile(
      DiscoveredPeer peer, ThemeData theme, ColorScheme colorScheme) {
    // Show the device key if available, otherwise fall back to address
    final peerKey = peer.deviceId ?? peer.fullAddress;
    final displayKey = peer.deviceId != null
        ? _formatKey(peer.deviceId!.replaceAll('-', ''))
        : peer.fullAddress;
    final isSelected = _selectedPeerKey == peerKey;

    return Card(
      elevation: 0,
      margin: const EdgeInsets.only(bottom: 8),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
        side: BorderSide(
          color: isSelected ? colorScheme.primary : theme.dividerColor,
          width: isSelected ? 1.5 : 0.5,
        ),
      ),
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: () {
          setState(() {
            _selectedPeerKey = peerKey;
            _selectedPeerName = peer.deviceName;
            _keyController.clear();
            _showManualKey = false;
          });
        },
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Row(
            children: [
              CircleAvatar(
                radius: 18,
                backgroundColor: colorScheme.primaryContainer,
                child: Icon(
                  peer.deviceName.toLowerCase().contains('phone') ||
                          peer.deviceName.toLowerCase().contains('mobile')
                      ? Icons.smartphone
                      : Icons.laptop,
                  color: colorScheme.primary,
                  size: 18,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(peer.deviceName,
                        style: theme.textTheme.bodyMedium
                            ?.copyWith(fontWeight: FontWeight.w600)),
                    Text(
                      displayKey,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: colorScheme.primary.withValues(alpha: 0.8),
                        fontFamily: 'monospace',
                        fontWeight: FontWeight.w600,
                        letterSpacing: 1.5,
                      ),
                    ),
                  ],
                ),
              ),
              if (isSelected)
                Icon(Icons.check_circle, color: colorScheme.primary, size: 22)
              else
                Icon(Icons.chevron_right,
                    color: colorScheme.onSurface.withValues(alpha: 0.3)),
            ],
          ),
        ),
      ),
    );
  }
}
