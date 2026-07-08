import 'package:flutter/material.dart';

class ReceivePage extends StatefulWidget {
  const ReceivePage({super.key});

  @override
  State<ReceivePage> createState() => _ReceivePageState();
}

class _ReceivePageState extends State<ReceivePage> {
  bool _listening = false;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Receive Files')),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(
              _listening ? Icons.wifi_tethering : Icons.wifi_tethering_off,
              size: 80,
              color: _listening ? Colors.green : Colors.grey,
            ),
            const SizedBox(height: 24),
            Text(
              _listening ? 'Listening on port 9877...' : 'Not listening',
              style: const TextStyle(fontSize: 18),
            ),
            const SizedBox(height: 32),
            ElevatedButton.icon(
              onPressed: () => setState(() => _listening = !_listening),
              icon: Icon(_listening ? Icons.stop : Icons.play_arrow),
              label: Text(_listening ? 'Stop' : 'Start Listening'),
              style: ElevatedButton.styleFrom(
                padding: const EdgeInsets.all(16),
                backgroundColor: _listening ? Colors.red : Colors.green,
                foregroundColor: Colors.white,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
