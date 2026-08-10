import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_zxing/flutter_zxing.dart';
import 'package:qrbeam_mobile/features/receive/receiver_scanner.dart';

void main() {
  test('receiver scanner uses high-detail QR settings', () {
    final scanner = buildReceiverScanner(onScan: (_) {});

    expect(scanner.tryHarder, isTrue);
    expect(scanner.tryDownscale, isTrue);
    expect(scanner.resolution, ResolutionPreset.veryHigh);
  });
}
