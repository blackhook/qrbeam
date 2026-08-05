import 'package:flutter_test/flutter_test.dart';
import 'package:qrbeam_mobile/main.dart';

void main() {
  testWidgets('bridge shell is visible', (tester) async {
    await tester.pumpWidget(const MyApp());

    expect(find.text('QRBeam receiver bridge ready'), findsOneWidget);
  });
}
