import 'dart:async';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart' as path_provider;
import 'src/home_page.dart';
import 'src/send_page.dart';
import 'src/receive_page.dart';
import 'src/history_page.dart';
import 'src/settings_page.dart';
import 'src/contacts_page.dart';
import 'src/device_identity_service.dart';
import 'src/transfer_service.dart';
import 'src/contacts_service.dart';
import 'src/rust/frb_generated.dart';

/// Global navigator key — allows showing dialogs from anywhere (not just pages).
final GlobalKey<NavigatorState> navigatorKey = GlobalKey<NavigatorState>();

/// Global device identity service instance.
final DeviceIdentityService deviceIdentity = DeviceIdentityService();

/// Global stream subscription for incoming transfer requests.
StreamSubscription<IncomingTransferRequest>? _globalIncomingSub;

void main() async {
  try {
    WidgetsFlutterBinding.ensureInitialized();
    await RustLib.init();
    await deviceIdentity.initialize();
    await ContactsService.instance.load();

    // Start the backend listener globally at startup.
    final docsDir = await path_provider.getApplicationDocumentsDirectory();
    await TransferService.instance.startListeningWithShortId(
      deviceIdentity.rawShortId,
      docsDir.path,
    );

    // Subscribe globally so incoming popups show on ANY page.
    _subscribeGlobalIncoming();

    runApp(const P2PTransferApp());
  } catch (e, stack) {
    runApp(MaterialApp(
      home: Scaffold(
        body: SafeArea(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(16),
            child: Text(
              'Startup Error:\n$e\n\n$stack',
              style: const TextStyle(color: Colors.red, fontSize: 14),
            ),
          ),
        ),
      ),
    ));
  }
}

void _subscribeGlobalIncoming() {
  _globalIncomingSub?.cancel();
  _globalIncomingSub =
      TransferService.instance.incomingRequests.listen(_onGlobalIncoming);
}

Future<void> _onGlobalIncoming(IncomingTransferRequest req) async {
  final ctx = navigatorKey.currentContext;
  if (ctx == null) {
    req.decline();
    return;
  }

  // Show the interactive accept/decline/progress sheet over whatever page is currently active.
  await showModalBottomSheet<void>(
    context: ctx,
    isDismissible: false,
    enableDrag: false,
    isScrollControlled: true,
    shape: const RoundedRectangleBorder(
      borderRadius: BorderRadius.vertical(top: Radius.circular(24)),
    ),
    builder: (_) => IncomingSheet(request: req),
  );
}

// ─── App ─────────────────────────────────────────────────────────────────────

class P2PTransferApp extends StatelessWidget {
  const P2PTransferApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'p2ptransfer',
      debugShowCheckedModeBanner: false,
      navigatorKey: navigatorKey,
      theme: _buildLightTheme(),
      darkTheme: _buildDarkTheme(),
      themeMode: ThemeMode.system,
      initialRoute: '/',
      routes: {
        '/': (context) => const HomePage(),
        '/send': (context) => const SendPage(),
        '/receive': (context) => const ReceivePage(),
        '/history': (context) => const HistoryPage(),
        '/contacts': (context) => const ContactsPage(),
        '/settings': (context) => const SettingsPage(),
      },
    );
  }

  ThemeData _buildLightTheme() {
    return ThemeData(
      useMaterial3: true,
      colorSchemeSeed: const Color(0xFF6750A4),
      brightness: Brightness.light,
      appBarTheme: const AppBarTheme(
        centerTitle: false,
        elevation: 0,
        scrolledUnderElevation: 1,
      ),
      cardTheme: CardThemeData(
        elevation: 1,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
        clipBehavior: Clip.antiAlias,
      ),
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(
          padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 14),
          shape:
              RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        border:
            OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
        contentPadding:
            const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
      ),
      snackBarTheme: SnackBarThemeData(
        behavior: SnackBarBehavior.floating,
        shape:
            RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
      ),
    );
  }

  ThemeData _buildDarkTheme() {
    return ThemeData(
      useMaterial3: true,
      colorSchemeSeed: const Color(0xFF6750A4),
      brightness: Brightness.dark,
      appBarTheme: const AppBarTheme(
        centerTitle: false,
        elevation: 0,
        scrolledUnderElevation: 1,
      ),
      cardTheme: CardThemeData(
        elevation: 1,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
        clipBehavior: Clip.antiAlias,
      ),
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(
          padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 14),
          shape:
              RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        border:
            OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
        contentPadding:
            const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
      ),
      snackBarTheme: SnackBarThemeData(
        behavior: SnackBarBehavior.floating,
        shape:
            RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
      ),
    );
  }
}
