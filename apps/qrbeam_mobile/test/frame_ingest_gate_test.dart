import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:qrbeam_mobile/features/receive/frame_ingest_gate.dart';

void main() {
  test('busy receiver drops a new camera frame instead of queueing it', () async {
    final release = Completer<void>();
    var calls = 0;
    final gate = FrameIngestGate((frame) async {
      calls++;
      await release.future;
    });

    final first = gate.submit([1, 2, 3]);
    await Future<void>.delayed(Duration.zero);
    final secondAccepted = await gate.submit([4, 5, 6]);

    expect(secondAccepted, isFalse);
    expect(calls, 1);
    release.complete();
    expect(await first, isTrue);
  });
}
