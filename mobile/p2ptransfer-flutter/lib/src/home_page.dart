import 'package:flutter/material.dart';
import '../main.dart';
import 'widgets/device_key_card.dart';

class HomePage extends StatelessWidget {
  const HomePage({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      body: CustomScrollView(
        slivers: [
          SliverAppBar(
            title: const Text('p2ptransfer'),
            actions: [
              IconButton(
                icon: const Icon(Icons.settings_outlined),
                onPressed: () => Navigator.pushNamed(context, '/settings'),
                tooltip: 'Settings',
              ),
            ],
          ),
          SliverPadding(
            padding: const EdgeInsets.symmetric(horizontal: 20),
            sliver: SliverList(
              delegate: SliverChildListDelegate([
                const SizedBox(height: 8),

                DeviceKeyCard(
                  deviceId: deviceIdentity.deviceId,
                  fullKeyHex: deviceIdentity.fullKeyHex,
                  deviceName: 'Mobile Device',
                ),

                const SizedBox(height: 24),

                Text(
                  'Quick Actions',
                  style: theme.textTheme.titleMedium?.copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 12),

                _ActionGrid(),

                const SizedBox(height: 24),

                Text(
                  'Status',
                  style: theme.textTheme.titleMedium?.copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 12),
                _StatusCard(),
                const SizedBox(height: 32),
              ]),
            ),
          ),
        ],
      ),
    );
  }
}

class _ActionGrid extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;

    return LayoutBuilder(
      builder: (context, constraints) {
        final itemWidth = (constraints.maxWidth - 12) / 2;
        return Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            _ActionTile(
              width: itemWidth,
              icon: Icons.upload_rounded,
              label: 'Send File',
              subtitle: 'Transfer files to a peer',
              color: colorScheme.primary,
              iconBgColor: colorScheme.primaryContainer,
              onTap: () => Navigator.pushNamed(context, '/send'),
            ),
            _ActionTile(
              width: itemWidth,
              icon: Icons.download_rounded,
              label: 'Receive',
              subtitle: 'Accept incoming files',
              color: colorScheme.tertiary,
              iconBgColor: colorScheme.tertiaryContainer,
              onTap: () => Navigator.pushNamed(context, '/receive'),
            ),
            _ActionTile(
              width: itemWidth,
              icon: Icons.history_rounded,
              label: 'History',
              subtitle: 'View past transfers',
              color: colorScheme.secondary,
              iconBgColor: colorScheme.secondaryContainer,
              onTap: () => Navigator.pushNamed(context, '/history'),
            ),
            _ActionTile(
              width: itemWidth,
              icon: Icons.settings_rounded,
              label: 'Settings',
              subtitle: 'Configure preferences',
              color: colorScheme.outline,
              iconBgColor: colorScheme.surfaceContainerHighest,
              onTap: () => Navigator.pushNamed(context, '/settings'),
            ),
          ],
        );
      },
    );
  }
}

class _ActionTile extends StatelessWidget {
  final double width;
  final IconData icon;
  final String label;
  final String subtitle;
  final Color color;
  final Color iconBgColor;
  final VoidCallback onTap;

  const _ActionTile({
    required this.width,
    required this.icon,
    required this.label,
    required this.subtitle,
    required this.color,
    required this.iconBgColor,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return SizedBox(
      width: width,
      child: Card(
        elevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(16),
          side: BorderSide(color: theme.dividerColor, width: 0.5),
        ),
        child: InkWell(
          borderRadius: BorderRadius.circular(16),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  padding: const EdgeInsets.all(10),
                  decoration: BoxDecoration(
                    color: iconBgColor,
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Icon(icon, color: color, size: 24),
                ),
                const SizedBox(height: 12),
                Text(
                  label,
                  style: theme.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onSurface.withOpacity(0.6),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _StatusCard extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;

    return Card(
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: theme.dividerColor, width: 0.5),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          children: [
            Container(
              width: 8,
              height: 8,
              decoration: const BoxDecoration(
                color: Colors.green,
                shape: BoxShape.circle,
              ),
            ),
            const SizedBox(width: 12),
            Icon(Icons.wifi, size: 20, color: colorScheme.onSurface.withOpacity(0.6)),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                'Ready to transfer',
                style: theme.textTheme.bodyMedium,
              ),
            ),
            Text(
              'v0.1.0',
              style: theme.textTheme.bodySmall?.copyWith(
                color: colorScheme.onSurface.withOpacity(0.4),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
