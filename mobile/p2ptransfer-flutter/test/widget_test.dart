import 'package:flutter_test/flutter_test.dart';
import 'package:p2ptransfer/main.dart';

void main() {
  testWidgets('App renders smoke test', (WidgetTester tester) async {
    await tester.pumpWidget(const P2PTransferApp());
    expect(find.text('p2ptransfer'), findsOneWidget);
  });
}
