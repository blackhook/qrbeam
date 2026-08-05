import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:qrbeam_mobile/src/rust/api/receiver.dart';
import 'package:qrbeam_mobile/src/rust/frb_generated.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(RustLib.init);

  testWidgets('Rust receiver starts in manifest phase', (tester) async {
    final receiver = await MobileReceiver.newInstance();
    final snapshot = await receiver.snapshot();

    expect(snapshot.phase, MobilePhase.waitingManifest);
    receiver.dispose();
  });
}
