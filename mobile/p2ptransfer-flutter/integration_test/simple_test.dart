import 'package:flutter_test/flutter_test.dart';
import 'package:p2ptransfer/main.dart';
import 'package:p2ptransfer/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => await RustLib.init());
  testWidgets('App launches', (WidgetTester tester) async {
    await tester.pumpWidget(const P2PTransferApp());
    await tester.pumpAndSettle();
    expect(find.byType(P2PTransferApp), findsOneWidget);
  });
}
