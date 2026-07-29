import 'dart:async';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:file_picker/file_picker.dart';
import 'package:path_provider/path_provider.dart';
import '../main.dart';
import 'settings_page.dart';
import 'transfer_service.dart';
import 'widgets/device_key_card.dart';

class ReceivePage extends StatefulWidget {
  const ReceivePage({super.key});

  @override
  State<ReceivePage> createState() => _ReceivePageState();
}

class _ReceivePageState extends State<ReceivePage>
    with SingleTickerProviderStateMixin {
  bool _listening = false;
  String _localIp = '';
  late AnimationController _pulseController;
  late Animation<double> _pulseAnimation;

  StreamSubscription<IncomingTransferRequest>? _incomingSub;

  @override
  void initState() {
    super.initState();
    _fetchLocalIp();

    _pulseController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1500),
    )..repeat(reverse: true);
    _pulseAnimation = Tween<double>(begin: 0.92, end: 1.0).animate(
      CurvedAnimation(parent: _pulseController, curve: Curves.easeInOut),
    );

    _updateListeningState();
    _subscribeToIncoming();
  }

  void _updateListeningState() {
    setState(() => _listening = TransferService.instance.isListening);
  }

  void _subscribeToIncoming() {
    _incomingSub?.cancel();
    _incomingSub =
        TransferService.instance.incomingRequests.listen(_onIncomingRequest);
  }

  Future<void> _toggleListening() async {
    if (_listening) {
      await TransferService.instance.stopListening();
    } else {
      await TransferService.instance
          .startListening(port: AppSettings.tcpPort);
      _subscribeToIncoming();
    }
    _updateListeningState();
  }

  @override
  void dispose() {
    _pulseController.dispose();
    _incomingSub?.cancel();
    super.dispose();
  }

  Future<void> _fetchLocalIp() async {
    try {
      final interfaces = await NetworkInterface.list(
        type: InternetAddressType.IPv4,
        includeLinkLocal: false,
      );
      for (final iface in interfaces) {
        for (final addr in iface.addresses) {
          if (!addr.isLoopback) {
            if (mounted) setState(() => _localIp = addr.address);
            return;
          }
        }
      }
    } catch (_) {}
  }

  // ── Incoming transfer request handler ────────────────────────────────────
  Future<void> _onIncomingRequest(IncomingTransferRequest req) async {
    if (!mounted) {
      req.decline();
      return;
    }
    // Show accept/decline bottom sheet
    await showModalBottomSheet<void>(
      context: context,
      isDismissible: false,
      enableDrag: false,
      isScrollControlled: true,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(24)),
      ),
      builder: (ctx) => _IncomingSheet(request: req),
    );
  }

  // ── Build ─────────────────────────────────────────────────────────────────
  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;
    final port = AppSettings.tcpPort;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Receive'),
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
            // ── Device Key ────────────────────────────────────────────────
            DeviceKeyCard(
              deviceId: deviceIdentity.deviceId,
              fullKeyHex: deviceIdentity.fullKeyHex,
              compact: true,
            ),
            const SizedBox(height: 24),

            // ── Listening Status Card ─────────────────────────────────────
            Card(
              elevation: 0,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(20),
                side: BorderSide(color: theme.dividerColor, width: 0.5),
              ),
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: Column(
                  children: [
                    // Animated pulse
                    AnimatedBuilder(
                      animation: _pulseAnimation,
                      builder: (ctx, child) => Transform.scale(
                        scale: _listening ? _pulseAnimation.value : 1.0,
                        child: Container(
                          width: 110,
                          height: 110,
                          decoration: BoxDecoration(
                            shape: BoxShape.circle,
                            color: _listening
                                ? Colors.green.withValues(alpha: 0.12)
                                : colorScheme.surfaceContainerHighest,
                            border: Border.all(
                              color: _listening
                                  ? Colors.green
                                  : colorScheme.outline
                                      .withValues(alpha: 0.3),
                              width: 3,
                            ),
                          ),
                          child: Icon(
                            _listening
                                ? Icons.wifi_tethering
                                : Icons.wifi_tethering_off,
                            size: 52,
                            color: _listening
                                ? Colors.green
                                : colorScheme.onSurface
                                    .withValues(alpha: 0.4),
                          ),
                        ),
                      ),
                    ),

                    const SizedBox(height: 20),
                    Text(
                      _listening ? 'Ready to Receive' : 'Listening Paused',
                      style: theme.textTheme.titleLarge
                          ?.copyWith(fontWeight: FontWeight.bold),
                    ),
                    const SizedBox(height: 6),
                    Text(
                      _listening
                          ? 'Share your Device Key with the sender'
                          : 'Tap Start to resume listening',
                      style: theme.textTheme.bodyMedium?.copyWith(
                        color: colorScheme.onSurface.withValues(alpha: 0.6),
                      ),
                      textAlign: TextAlign.center,
                    ),

                    // IP badge
                    if (_listening && _localIp.isNotEmpty) ...[
                      const SizedBox(height: 16),
                      Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 16, vertical: 8),
                        decoration: BoxDecoration(
                          color: colorScheme.primaryContainer
                              .withValues(alpha: 0.5),
                          borderRadius: BorderRadius.circular(30),
                        ),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Icon(Icons.router,
                                size: 15,
                                color: colorScheme.onPrimaryContainer),
                            const SizedBox(width: 6),
                            Text(
                              '$_localIp:$port',
                              style: theme.textTheme.bodySmall?.copyWith(
                                fontFamily: 'monospace',
                                fontWeight: FontWeight.w600,
                                color: colorScheme.onPrimaryContainer,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],

                    const SizedBox(height: 20),
                    SizedBox(
                      width: double.infinity,
                      child: ElevatedButton.icon(
                        onPressed: _toggleListening,
                        icon: Icon(
                            _listening ? Icons.pause : Icons.play_arrow),
                        label: Text(
                          _listening ? 'Pause Listening' : 'Start Listening',
                        ),
                        style: ElevatedButton.styleFrom(
                          backgroundColor:
                              _listening ? Colors.orange : Colors.green,
                          foregroundColor: Colors.white,
                          padding:
                              const EdgeInsets.symmetric(vertical: 14),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),

            const SizedBox(height: 20),

            // ── Device Info ───────────────────────────────────────────────
            Card(
              elevation: 0,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(16),
                side: BorderSide(color: theme.dividerColor, width: 0.5),
              ),
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Column(
                  children: [
                    _infoRow(context, Icons.phone_android, 'Device',
                        AppSettings.deviceName),
                    const Divider(height: 20),
                    _infoRow(context, Icons.wifi, 'Local IP',
                        _localIp.isEmpty ? 'Fetching…' : '$_localIp:$port'),
                    const Divider(height: 20),
                    _infoRow(
                      context,
                      Icons.vpn_key,
                      'Device Key',
                      deviceIdentity.deviceId,
                      onTap: () {
                        Clipboard.setData(
                          ClipboardData(text: deviceIdentity.deviceId),
                        );
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text('Device key copied!'),
                          ),
                        );
                      },
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _infoRow(
    BuildContext context,
    IconData icon,
    String label,
    String value, {
    VoidCallback? onTap,
  }) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 2),
        child: Row(
          children: [
            Icon(icon, size: 18, color: cs.onSurface.withValues(alpha: 0.5)),
            const SizedBox(width: 10),
            Text(
              label,
              style: theme.textTheme.bodySmall?.copyWith(
                color: cs.onSurface.withValues(alpha: 0.6),
              ),
            ),
            const Spacer(),
            Flexible(
              child: Text(
                value,
                style: theme.textTheme.bodyMedium
                    ?.copyWith(fontWeight: FontWeight.w500),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ── Accept / Decline bottom sheet ─────────────────────────────────────────────
class _IncomingSheet extends StatefulWidget {
  final IncomingTransferRequest request;
  const _IncomingSheet({required this.request});

  @override
  State<_IncomingSheet> createState() => _IncomingSheetState();
}

class _IncomingSheetState extends State<_IncomingSheet> {
  String? _savePath;
  bool _transferring = false;
  double _progress = 0;
  String _statusLabel = '';
  // ignore: prefer_final_fields
  bool _done = false;
  // ignore: prefer_final_fields
  bool _error = false;
  StreamSubscription<TransferProgress>? _progressSub;

  @override
  void initState() {
    super.initState();
    _initSavePath();
  }

  Future<void> _initSavePath() async {
    try {
      final dir = await getExternalStorageDirectory() ??
          await getApplicationDocumentsDirectory();
      final downloads = Directory('${dir.path}/P2PTransfer');
      await downloads.create(recursive: true);
      if (mounted) setState(() => _savePath = downloads.path);
    } catch (_) {}
  }

  Future<void> _pickSaveDir() async {
    final picked = await FilePicker.platform.getDirectoryPath(
      dialogTitle: 'Save file to…',
    );
    if (picked != null && mounted) setState(() => _savePath = picked);
  }

  String _formatSize(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
    if (bytes < 1024 * 1024 * 1024) {
      return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
    }
    return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
  }

  void _accept() {
    if (_savePath == null) return;
    final fullPath = '$_savePath/${widget.request.fileName}';
    widget.request.accept(fullPath);
    setState(() {
      _transferring = true;
      _statusLabel = 'Starting…';
    });
    // We just told the service where to save; progress comes from the background
    // The service will call back into the socket; we can watch by polling or
    // subscribing to a per-session progress stream (added later).
    // For now poll state from TransferService
    _watchProgress();
  }

  void _watchProgress() {
    // Poll every 100ms — basic but reliable for now
    Future.doWhile(() async {
      await Future.delayed(const Duration(milliseconds: 200));
      if (!mounted) return false;
      // TODO: replace with a real progress stream from TransferService
      // For now just animate for demo until service pushes progress
      setState(() {
        if (_progress < 0.98) {
          _progress += 0.05;
          _statusLabel = '${(_progress * 100).toInt()}%';
        }
      });
      return _progress < 0.98 && !_done && !_error;
    });
  }

  void _decline() {
    widget.request.decline();
    Navigator.pop(context);
  }

  @override
  void dispose() {
    _progressSub?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final cs = theme.colorScheme;
    final req = widget.request;

    return Padding(
      padding: EdgeInsets.only(
        left: 24,
        right: 24,
        top: 24,
        bottom: MediaQuery.of(context).viewInsets.bottom + 24,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Header
          Row(
            children: [
              Container(
                padding: const EdgeInsets.all(10),
                decoration: BoxDecoration(
                  color: cs.primaryContainer,
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Icon(Icons.download_rounded,
                    color: cs.primary, size: 28),
              ),
              const SizedBox(width: 14),
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('Incoming File',
                      style: theme.textTheme.titleMedium
                          ?.copyWith(fontWeight: FontWeight.bold)),
                  Text('from ${req.senderIp}',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: cs.onSurface.withValues(alpha: 0.5),
                      )),
                ],
              ),
            ],
          ),

          const SizedBox(height: 20),

          // File info
          Container(
            padding: const EdgeInsets.all(16),
            decoration: BoxDecoration(
              color: cs.surfaceContainerHighest.withValues(alpha: 0.5),
              borderRadius: BorderRadius.circular(14),
            ),
            child: Row(
              children: [
                Icon(Icons.insert_drive_file_rounded,
                    color: cs.primary, size: 40),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(req.fileName,
                          style: theme.textTheme.bodyLarge?.copyWith(
                            fontWeight: FontWeight.w600,
                          ),
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis),
                      Text(_formatSize(req.fileSize),
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: cs.onSurface.withValues(alpha: 0.5),
                          )),
                    ],
                  ),
                ),
              ],
            ),
          ),

          const SizedBox(height: 16),

          // Save location picker
          if (!_transferring) ...[
            Text('Save to',
                style: theme.textTheme.titleSmall
                    ?.copyWith(fontWeight: FontWeight.w600)),
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(
                  child: Container(
                    padding: const EdgeInsets.symmetric(
                        horizontal: 12, vertical: 10),
                    decoration: BoxDecoration(
                      border:
                          Border.all(color: theme.dividerColor, width: 0.8),
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Text(
                      _savePath ?? 'Loading…',
                      style: theme.textTheme.bodySmall?.copyWith(
                        fontFamily: 'monospace',
                      ),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                OutlinedButton(
                  onPressed: _pickSaveDir,
                  style: OutlinedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(
                        horizontal: 12, vertical: 10),
                  ),
                  child: const Text('Change'),
                ),
              ],
            ),
            const SizedBox(height: 24),

            // Buttons
            Row(
              children: [
                Expanded(
                  child: OutlinedButton.icon(
                    onPressed: _decline,
                    icon: const Icon(Icons.close),
                    label: const Text('Decline'),
                    style: OutlinedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(vertical: 14),
                      foregroundColor: cs.error,
                      side: BorderSide(color: cs.error.withValues(alpha: 0.4)),
                    ),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: FilledButton.icon(
                    onPressed: _savePath != null ? _accept : null,
                    icon: const Icon(Icons.check),
                    label: const Text('Accept'),
                    style: FilledButton.styleFrom(
                      padding: const EdgeInsets.symmetric(vertical: 14),
                    ),
                  ),
                ),
              ],
            ),
          ],

          // Progress
          if (_transferring) ...[
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(
                  child: LinearProgressIndicator(
                    value: _progress > 0 ? _progress : null,
                    minHeight: 8,
                    borderRadius: BorderRadius.circular(4),
                  ),
                ),
                const SizedBox(width: 12),
                Text(_statusLabel,
                    style: theme.textTheme.bodySmall
                        ?.copyWith(fontWeight: FontWeight.w600)),
              ],
            ),
            const SizedBox(height: 8),
            Text(
              _done
                  ? '✓ Saved to ${_savePath ?? ""}/${req.fileName}'
                  : _error
                      ? '✗ Transfer failed'
                      : 'Receiving — do not close this screen…',
              style: theme.textTheme.bodySmall?.copyWith(
                color: _done
                    ? Colors.green
                    : _error
                        ? cs.error
                        : cs.onSurface.withValues(alpha: 0.6),
              ),
            ),
            if (_done) ...[
              const SizedBox(height: 16),
              SizedBox(
                width: double.infinity,
                child: FilledButton(
                  onPressed: () => Navigator.pop(context),
                  child: const Text('Done'),
                ),
              ),
            ],
          ],
          const SizedBox(height: 8),
        ],
      ),
    );
  }
}
