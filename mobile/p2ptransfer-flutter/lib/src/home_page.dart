import 'package:flutter/material.dart';

class HomePage extends StatelessWidget {
  const HomePage({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('p2ptransfer')),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.swap_horiz, size: 80, color: Color(0xFF1a1a2e)),
            const SizedBox(height: 24),
            const Text('Secure P2P File Transfer',
                style: TextStyle(fontSize: 20, fontWeight: FontWeight.bold)),
            const SizedBox(height: 32),
            _buildButton(context, 'Send File', Icons.upload_file, '/send'),
            const SizedBox(height: 12),
            _buildButton(context, 'Receive Files', Icons.download, '/receive'),
            const SizedBox(height: 12),
            _buildButton(context, 'History', Icons.history, '/history'),
          ],
        ),
      ),
    );
  }

  Widget _buildButton(BuildContext context, String label, IconData icon, String route) {
    return SizedBox(
      width: 240,
      child: ElevatedButton.icon(
        onPressed: () => Navigator.pushNamed(context, route),
        icon: Icon(icon),
        label: Text(label),
        style: ElevatedButton.styleFrom(padding: const EdgeInsets.all(16)),
      ),
    );
  }
}
