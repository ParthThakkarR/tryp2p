import 'package:flutter/material.dart';
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
  final _peerController = TextEditingController(text: '192.168.1.100:9877');
  final PeerDiscoveryService _discoveryService = PeerDiscoveryService();
  List<DiscoveredPeer> _discoveredPeers = [];
  bool _isScanning = false;
  bool _showManualAddress = false;
  String? _selectedPeerAddress;

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
    _peerController.dispose();
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

  void _sendTransfer() {
    final targetAddress = _selectedPeerAddress ?? _peerController.text.trim();
    final compression = AppSettings.compressionLevel;

    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text('Sending to $targetAddress (Compression: Level $compression)...'),
        backgroundColor: Colors.blue[800],
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

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;
    final activeTarget = _selectedPeerAddress ?? _peerController.text.trim();
    final hasValidTarget = activeTarget.isNotEmpty;
    final hasFile = _filePath != null;

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
            // Compact device key
            DeviceKeyCard(
              deviceId: deviceIdentity.deviceId,
              fullKeyHex: deviceIdentity.fullKeyHex,
              compact: true,
            ),
            const SizedBox(height: 20),

            // File Selection
            Text('Select File', style: theme.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w600,
            )),
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
                              style: theme.textTheme.bodyMedium?.copyWith(
                                fontWeight: FontWeight.w500,
                              ),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                            if (_fileSize != null)
                              Text(
                                _formatFileSize(_fileSize!),
                                style: theme.textTheme.bodySmall?.copyWith(
                                  color: colorScheme.onSurface.withOpacity(0.5),
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

            // Target Section
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text('Target Device', style: theme.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w600,
                )),
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
                    label: const Text('Refresh'),
                  ),
              ],
            ),
            const SizedBox(height: 8),

            // Discovered Peers
            if (_discoveredPeers.isNotEmpty)
              ..._discoveredPeers.map((peer) => _buildPeerTile(peer, theme, colorScheme)),

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
                        Icon(Icons.wifi_find, size: 40,
                            color: colorScheme.onSurface.withOpacity(0.2)),
                        const SizedBox(height: 8),
                        Text('No nearby devices found',
                            style: theme.textTheme.bodyMedium?.copyWith(
                              color: colorScheme.onSurface.withOpacity(0.5),
                            )),
                        Text('Ensure both devices are on the same Wi-Fi network',
                            style: theme.textTheme.bodySmall?.copyWith(
                              color: colorScheme.onSurface.withOpacity(0.3),
                            )),
                      ],
                    ),
                  ),
                ),
              ),

            // Manual entry toggle
            const SizedBox(height: 8),
            TextButton.icon(
              onPressed: () => setState(() => _showManualAddress = !_showManualAddress),
              icon: Icon(
                _showManualAddress ? Icons.expand_less : Icons.expand_more,
                size: 18,
              ),
              label: Text(
                _showManualAddress ? 'Hide manual entry' : 'Or enter address manually',
              ),
            ),

            if (_showManualAddress) ...[
              const SizedBox(height: 8),
              TextField(
                controller: _peerController,
                onChanged: (val) => setState(() => _selectedPeerAddress = val.trim()),
                decoration: const InputDecoration(
                  labelText: 'IP Address:Port',
                  hintText: 'e.g. 192.168.1.100:9877',
                  prefixIcon: Icon(Icons.lan),
                ),
              ),
            ],

            const SizedBox(height: 24),

            // Compression info
            Row(
              children: [
                Icon(Icons.speed, size: 16,
                    color: colorScheme.onSurface.withOpacity(0.4)),
                const SizedBox(width: 6),
                Text(
                  'Compression: Level ${AppSettings.compressionLevel}',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: colorScheme.onSurface.withOpacity(0.5),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),

            // Send Button
            SizedBox(
              width: double.infinity,
              child: FilledButton.icon(
                onPressed: (hasFile && hasValidTarget) ? _sendTransfer : null,
                icon: const Icon(Icons.send_rounded),
                label: const Text('Send File', style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600)),
                style: FilledButton.styleFrom(
                  padding: const EdgeInsets.symmetric(vertical: 16),
                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
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
    final isSelected = _selectedPeerAddress == peer.fullAddress;
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
            _selectedPeerAddress = peer.fullAddress;
            _peerController.text = peer.fullAddress;
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
                        style: theme.textTheme.bodyMedium?.copyWith(
                            fontWeight: FontWeight.w600)),
                    Text(peer.fullAddress,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: colorScheme.onSurface.withOpacity(0.5),
                          fontFamily: 'monospace',
                        )),
                  ],
                ),
              ),
              if (isSelected)
                Icon(Icons.check_circle, color: colorScheme.primary, size: 22)
              else
                Icon(Icons.chevron_right,
                    color: colorScheme.onSurface.withOpacity(0.3)),
            ],
          ),
        ),
      ),
    );
  }
}
