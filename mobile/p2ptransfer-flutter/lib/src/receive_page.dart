import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../main.dart';
import 'settings_page.dart';
import 'widgets/device_key_card.dart';

class ReceivePage extends StatefulWidget {
  const ReceivePage({super.key});

  @override
  State<ReceivePage> createState() => _ReceivePageState();
}

class _ReceivePageState extends State<ReceivePage>
    with SingleTickerProviderStateMixin {
  bool _listening = true;
  String _localIp = 'Fetching IP...';
  late AnimationController _pulseController;
  late Animation<double> _pulseAnimation;

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
  }

  @override
  void dispose() {
    _pulseController.dispose();
    super.dispose();
  }

  Future<void> _fetchLocalIp() async {
    try {
      final interfaces = await NetworkInterface.list(
        type: InternetAddressType.IPv4,
        includeLinkLocal: false,
      );
      for (final interface in interfaces) {
        for (final addr in interface.addresses) {
          if (!addr.isLoopback) {
            if (mounted) setState(() => _localIp = addr.address);
            return;
          }
        }
      }
    } catch (_) {}
    if (mounted) setState(() => _localIp = '127.0.0.1');
  }

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
                Navigator.pushNamed(context, '/settings').then((_) => setState(() {})),
            tooltip: 'Settings',
          ),
        ],
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Device Key Card (compact)
            DeviceKeyCard(
              deviceId: deviceIdentity.deviceId,
              fullKeyHex: deviceIdentity.fullKeyHex,
              compact: true,
            ),

            const SizedBox(height: 24),

            // Listening Status Card
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
                    // Animated status icon
                    AnimatedBuilder(
                      animation: _pulseAnimation,
                      builder: (context, child) {
                        return Transform.scale(
                          scale: _listening ? _pulseAnimation.value : 1.0,
                          child: Container(
                            width: 120,
                            height: 120,
                            decoration: BoxDecoration(
                              shape: BoxShape.circle,
                              color: _listening
                                  ? Colors.green.withOpacity(0.1)
                                  : colorScheme.surfaceContainerHighest,
                              border: Border.all(
                                color: _listening
                                    ? Colors.green
                                    : colorScheme.outline.withOpacity(0.3),
                                width: 3,
                              ),
                            ),
                            child: Icon(
                              _listening
                                  ? Icons.wifi_tethering
                                  : Icons.wifi_tethering_off,
                              size: 56,
                              color: _listening
                                  ? Colors.green
                                  : colorScheme.onSurface.withOpacity(0.4),
                            ),
                          ),
                        );
                      },
                    ),
                    const SizedBox(height: 20),

                    Text(
                      _listening ? 'Ready to Receive' : 'Listening Paused',
                      style: theme.textTheme.titleLarge?.copyWith(
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      _listening
                          ? 'Share your Device Key with the sender'
                          : 'Tap Start to resume listening',
                      style: theme.textTheme.bodyMedium?.copyWith(
                        color: colorScheme.onSurface.withOpacity(0.6),
                      ),
                      textAlign: TextAlign.center,
                    ),

                    const SizedBox(height: 24),

                    // Listening info badge
                    if (_listening)
                      Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 16, vertical: 10),
                        decoration: BoxDecoration(
                          color: colorScheme.primaryContainer.withOpacity(0.5),
                          borderRadius: BorderRadius.circular(30),
                        ),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Icon(Icons.router,
                                size: 16, color: colorScheme.onPrimaryContainer),
                            const SizedBox(width: 8),
                            Text(
                              'Listening on port $port',
                              style: theme.textTheme.bodySmall?.copyWith(
                                fontWeight: FontWeight.w500,
                                color: colorScheme.onPrimaryContainer,
                              ),
                            ),
                          ],
                        ),
                      ),

                    const SizedBox(height: 20),

                    // Toggle button
                    SizedBox(
                      width: double.infinity,
                      child: ElevatedButton.icon(
                        onPressed: () =>
                            setState(() => _listening = !_listening),
                        icon: Icon(_listening ? Icons.pause : Icons.play_arrow),
                        label: Text(
                            _listening ? 'Pause Listening' : 'Start Listening'),
                        style: ElevatedButton.styleFrom(
                          backgroundColor:
                              _listening ? Colors.orange : Colors.green,
                          foregroundColor: Colors.white,
                          padding: const EdgeInsets.symmetric(vertical: 14),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),

            const SizedBox(height: 20),

            // Device Info
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
                    _infoRow(
                      context,
                      Icons.phone_android,
                      'Device',
                      AppSettings.deviceName,
                    ),
                    const Divider(height: 20),
                    _infoRow(
                      context,
                      Icons.wifi,
                      'Local IP',
                      '$_localIp:$port',
                    ),
                    const Divider(height: 20),
                    _infoRow(
                      context,
                      Icons.vpn_key,
                      'Device Key',
                      deviceIdentity.deviceId,
                      onTap: () {
                        Clipboard.setData(
                            ClipboardData(text: deviceIdentity.deviceId));
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(content: Text('Device ID copied!')),
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
            Icon(icon, size: 18,
                color: cs.onSurface.withOpacity(0.5)),
            const SizedBox(width: 10),
            Text(
              label,
              style: theme.textTheme.bodySmall?.copyWith(
                color: cs.onSurface.withOpacity(0.6),
              ),
            ),
            const Spacer(),
            Flexible(
              child: Text(
                value,
                style: theme.textTheme.bodyMedium?.copyWith(
                  fontWeight: FontWeight.w500,
                  fontFamily: label == 'IP' ? 'monospace' : null,
                ),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
