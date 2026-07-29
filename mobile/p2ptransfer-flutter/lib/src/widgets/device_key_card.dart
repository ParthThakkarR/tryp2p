import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// A polished card that displays the device's short key prominently,
/// with a copy button.
class DeviceKeyCard extends StatelessWidget {
  final String deviceId;
  final String fullKeyHex;
  final String deviceName;
  final bool compact;

  const DeviceKeyCard({
    super.key,
    required this.deviceId,
    required this.fullKeyHex,
    this.deviceName = '',
    this.compact = false,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;

    if (compact) {
      return _buildCompact(context, theme, colorScheme);
    }
    return _buildFull(context, theme, colorScheme);
  }

  Widget _buildFull(BuildContext context, ThemeData theme, ColorScheme colorScheme) {
    return Card(
      elevation: 2,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(20)),
      child: Container(
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(20),
          gradient: LinearGradient(
            colors: [
              colorScheme.primaryContainer,
              colorScheme.primaryContainer.withValues(alpha: 0.6),
            ],
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
          ),
        ),
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Header row
            Row(
              children: [
                Container(
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    color: colorScheme.primary.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Icon(Icons.vpn_key, color: colorScheme.primary, size: 22),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Your Device Key',
                        style: theme.textTheme.titleSmall?.copyWith(
                          color: colorScheme.onPrimaryContainer.withValues(alpha: 0.7),
                        ),
                      ),
                      if (deviceName.isNotEmpty)
                        Text(
                          deviceName,
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: colorScheme.onPrimaryContainer.withValues(alpha: 0.5),
                          ),
                        ),
                    ],
                  ),
                ),
                _buildCopyButton(context, colorScheme, deviceId, 'Device ID copied!'),
              ],
            ),
            const SizedBox(height: 16),
            // Large device ID display
            Center(
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
                decoration: BoxDecoration(
                  color: colorScheme.surface.withValues(alpha: 0.5),
                  borderRadius: BorderRadius.circular(16),
                  border: Border.all(
                    color: colorScheme.primary.withValues(alpha: 0.2),
                    width: 1,
                  ),
                ),
                child: Text(
                  deviceId,
                  style: theme.textTheme.headlineMedium?.copyWith(
                    fontWeight: FontWeight.w900,
                    letterSpacing: 6,
                    color: colorScheme.onPrimaryContainer,
                  ),
                ),
              ),
            ),
            const SizedBox(height: 12),
            // Full key (subtle)
            Center(
              child: GestureDetector(
                onLongPress: () {
                  Clipboard.setData(ClipboardData(text: fullKeyHex));
                  ScaffoldMessenger.of(context).showSnackBar(
                    const SnackBar(
                      content: Text('Full key copied!'),
                      duration: Duration(seconds: 2),
                    ),
                  );
                },
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                  decoration: BoxDecoration(
                    color: colorScheme.surface.withValues(alpha: 0.3),
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(Icons.key, size: 12, color: colorScheme.onPrimaryContainer.withValues(alpha: 0.4)),
                      const SizedBox(width: 6),
                      Flexible(
                        child: Text(
                          fullKeyHex.length > 16
                              ? '${fullKeyHex.substring(0, 16)}...${fullKeyHex.substring(fullKeyHex.length - 8)}'
                              : fullKeyHex,
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: colorScheme.onPrimaryContainer.withValues(alpha: 0.4),
                            fontFamily: 'monospace',
                            fontSize: 10,
                          ),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildCompact(BuildContext context, ThemeData theme, ColorScheme colorScheme) {
    return Card(
      elevation: 1,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: () {
          Clipboard.setData(ClipboardData(text: deviceId));
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(
              content: Text('Device ID copied!'),
              duration: Duration(seconds: 2),
            ),
          );
        },
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Row(
            children: [
              Container(
                padding: const EdgeInsets.all(6),
                decoration: BoxDecoration(
                  color: colorScheme.primary.withValues(alpha: 0.12),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Icon(Icons.vpn_key, color: colorScheme.primary, size: 16),
              ),
              const SizedBox(width: 10),
              Text('Device Key', style: theme.textTheme.bodySmall?.copyWith(
                color: colorScheme.onSurface.withValues(alpha: 0.6),
              )),
              const Spacer(),
              Text(
                deviceId,
                style: theme.textTheme.titleMedium?.copyWith(
                  fontWeight: FontWeight.w800,
                  letterSpacing: 3,
                  color: colorScheme.primary,
                ),
              ),
              const SizedBox(width: 6),
              Icon(Icons.copy, size: 16, color: colorScheme.onSurface.withValues(alpha: 0.4)),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildCopyButton(BuildContext context, ColorScheme colorScheme, String text, String message) {
    return IconButton(
      icon: const Icon(Icons.copy_rounded, size: 20),
      onPressed: () {
        Clipboard.setData(ClipboardData(text: text));
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(message),
            duration: const Duration(seconds: 2),
            behavior: SnackBarBehavior.floating,
          ),
        );
      },
      tooltip: 'Copy device ID',
      style: IconButton.styleFrom(
        backgroundColor: colorScheme.primary.withValues(alpha: 0.1),
        foregroundColor: colorScheme.onPrimaryContainer,
      ),
    );
  }
}
