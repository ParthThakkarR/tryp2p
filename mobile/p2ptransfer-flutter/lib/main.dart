import 'package:flutter/material.dart';
import 'src/home_page.dart';
import 'src/send_page.dart';
import 'src/receive_page.dart';
import 'src/history_page.dart';

void main() {
  runApp(const P2PTransferApp());
}

class P2PTransferApp extends StatelessWidget {
  const P2PTransferApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'p2ptransfer',
      theme: ThemeData(
        colorSchemeSeed: const Color(0xFF1a1a2e),
        useMaterial3: true,
        brightness: Brightness.light,
      ),
      initialRoute: '/',
      routes: {
        '/': (context) => const HomePage(),
        '/send': (context) => const SendPage(),
        '/receive': (context) => const ReceivePage(),
        '/history': (context) => const HistoryPage(),
      },
    );
  }
}
