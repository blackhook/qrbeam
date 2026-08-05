import 'package:flutter_test/flutter_test.dart';
import 'package:qrbeam_mobile/features/receive/receive_view_model.dart';
import 'package:qrbeam_mobile/src/rust/api/receiver.dart';

MobileSnapshot snapshot({
  required MobilePhase phase,
  List<MobileBlockState> blocks = const [],
}) => MobileSnapshot(
  phase: phase,
  filename: phase == MobilePhase.waitingManifest ? null : 'sample.bin',
  totalBytes: phase == MobilePhase.waitingManifest ? null : BigInt.from(4096),
  blocks: blocks,
  lastFrameIndex: BigInt.from(42),
);

void main() {
  test('progress includes partial symbols and ignores failed blocks', () {
    final state = ReceiveViewModel.fromSnapshot(
      snapshot(
        phase: MobilePhase.receiving,
        blocks: const [
          MobileBlockState(
            kind: MobileBlockKind.complete,
            unique: 0,
            required_: 0,
          ),
          MobileBlockState(
            kind: MobileBlockKind.partial,
            unique: 50,
            required_: 100,
          ),
          MobileBlockState(
            kind: MobileBlockKind.missing,
            unique: 0,
            required_: 0,
          ),
          MobileBlockState(
            kind: MobileBlockKind.failed,
            unique: 100,
            required_: 100,
          ),
        ],
      ),
    );

    expect(state.progress, 0.375);
  });

  test('receiving progress is capped at 99 percent until Rust completes', () {
    const completeBlock = MobileBlockState(
      kind: MobileBlockKind.complete,
      unique: 0,
      required_: 0,
    );

    expect(
      ReceiveViewModel.fromSnapshot(
        snapshot(phase: MobilePhase.receiving, blocks: const [completeBlock]),
      ).progress,
      0.99,
    );
    expect(
      ReceiveViewModel.fromSnapshot(
        snapshot(phase: MobilePhase.complete, blocks: const [completeBlock]),
      ).progress,
      1,
    );
  });

  test('throughput window reports instant and recent ten-second bandwidth', () {
    final window = ThroughputWindow();
    final start = DateTime.utc(2026, 8, 5, 10);

    window.record(1000, start);
    window.record(3000, start.add(const Duration(seconds: 2)));

    expect(window.instantBytesPerSecond, 1500);
    expect(
      window.stableBytesPerSecond(start.add(const Duration(seconds: 2))),
      2000,
    );
  });
}
