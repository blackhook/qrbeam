import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:qrbeam_mobile/features/receive/receive_screen.dart';
import 'package:qrbeam_mobile/features/receive/receive_view_model.dart';
import 'package:qrbeam_mobile/src/rust/api/receiver.dart';

Widget appFor(MobileSnapshot snapshot) => MaterialApp(
  home: ReceiveScreen(
    state: ReceiveViewModel.fromSnapshot(snapshot),
    scanner: const ColoredBox(key: Key('scanner'), color: Colors.black),
    onShare: () {},
  ),
);

void main() {
  testWidgets('waiting screen asks the user to align the computer QR code', (
    tester,
  ) async {
    await tester.pumpWidget(
      appFor(
        const MobileSnapshot(
          phase: MobilePhase.waitingManifest,
          blocks: [],
        ),
      ),
    );

    expect(find.text('等待文件清单'), findsOneWidget);
    expect(find.text('对准电脑屏幕上的二维码'), findsOneWidget);
    expect(find.byKey(const Key('scanner')), findsOneWidget);
  });

  testWidgets('receiving and complete screens expose block status and save', (
    tester,
  ) async {
    const blocks = [
      MobileBlockState(
        kind: MobileBlockKind.complete,
        unique: 0,
        required_: 0,
      ),
      MobileBlockState(
        kind: MobileBlockKind.partial,
        unique: 2,
        required_: 4,
      ),
      MobileBlockState(
        kind: MobileBlockKind.failed,
        unique: 4,
        required_: 4,
      ),
    ];
    await tester.pumpWidget(
      appFor(
        MobileSnapshot(
          phase: MobilePhase.receiving,
          filename: 'sample.bin',
          totalBytes: BigInt.from(4096),
          blocks: blocks,
          lastFrameIndex: BigInt.from(42),
        ),
      ),
    );

    expect(find.text('sample.bin'), findsOneWidget);
    expect(find.byKey(const Key('block-0')), findsOneWidget);
    expect(find.byKey(const Key('block-1')), findsOneWidget);
    expect(find.byKey(const Key('block-2')), findsOneWidget);
    expect(find.textContaining('帧 42'), findsOneWidget);

    await tester.pumpWidget(
      appFor(
        MobileSnapshot(
          phase: MobilePhase.complete,
          filename: 'sample.bin',
          totalBytes: BigInt.from(4096),
          blocks: blocks,
        ),
      ),
    );

    expect(find.text('接收完成'), findsOneWidget);
    expect(find.text('保存或分享'), findsOneWidget);
  });
}
