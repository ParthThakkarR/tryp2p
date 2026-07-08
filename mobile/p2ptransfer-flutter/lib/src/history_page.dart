import 'package:flutter/material.dart';

class HistoryPage extends StatelessWidget {
  const HistoryPage({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Transfer History')),
      body: const Center(
        child: Text('Transfer history will appear here once flutter_rust_bridge is connected.',
            style: TextStyle(fontSize: 16, color: Colors.grey)),
      ),
    );
  }
}
