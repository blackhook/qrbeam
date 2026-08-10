import 'package:flutter/material.dart';
import 'package:qrbeam_mobile/features/receive/receive_view_model.dart';
import 'package:qrbeam_mobile/src/rust/api/receiver.dart';

class ReceiveScreen extends StatelessWidget {
  const ReceiveScreen({
    required this.state,
    required this.scanner,
    required this.onShare,
    super.key,
  });

  final ReceiveViewModel state;
  final Widget scanner;
  final VoidCallback onShare;

  @override
  Widget build(BuildContext context) {
    final snapshot = state.snapshot;
    return Scaffold(
      backgroundColor: const Color(0xff0c0d0f),
      body: SafeArea(
        child: Column(
          children: [
            Expanded(
              flex: 5,
              child: Stack(
                fit: StackFit.expand,
                children: [
                  scanner,
                  IgnorePointer(
                    child: Center(
                      child: Container(
                        width: 240,
                        height: 240,
                        decoration: BoxDecoration(
                          border: Border.all(color: Colors.white, width: 2),
                          borderRadius: BorderRadius.circular(24),
                        ),
                      ),
                    ),
                  ),
                  const Positioned(
                    left: 0,
                    right: 0,
                    bottom: 18,
                    child: Text(
                      '对准电脑屏幕上的二维码',
                      textAlign: TextAlign.center,
                      style: TextStyle(color: Colors.white),
                    ),
                  ),
                ],
              ),
            ),
            Expanded(
              flex: 6,
              child: DecoratedBox(
                decoration: const BoxDecoration(
                  color: Color(0xfff6f5f1),
                  borderRadius: BorderRadius.vertical(top: Radius.circular(28)),
                ),
                child: SingleChildScrollView(
                  padding: const EdgeInsets.fromLTRB(20, 20, 20, 28),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        state.title,
                        style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      if (snapshot.filename case final filename?) ...[
                        const SizedBox(height: 8),
                        Text(filename, style: Theme.of(context).textTheme.titleMedium),
                        Text(_formatBytes(snapshot.totalBytes)),
                      ],
                      const SizedBox(height: 16),
                      LinearProgressIndicator(
                        value: state.progress,
                        minHeight: 8,
                        borderRadius: BorderRadius.circular(8),
                        color: const Color(0xff177245),
                        backgroundColor: const Color(0xffdeddd8),
                      ),
                      const SizedBox(height: 8),
                      Text('${(state.progress * 100).toStringAsFixed(1)}%'),
                      if (snapshot.blocks.isNotEmpty) ...[
                        const SizedBox(height: 18),
                        Wrap(
                          spacing: 5,
                          runSpacing: 5,
                          children: [
                            for (var index = 0; index < snapshot.blocks.length; index++)
                              Tooltip(
                                message: '区块 ${index + 1}',
                                child: Container(
                                  key: Key('block-$index'),
                                  width: 18,
                                  height: 18,
                                  decoration: BoxDecoration(
                                    color: _blockColor(snapshot.blocks[index].kind),
                                    borderRadius: BorderRadius.circular(4),
                                  ),
                                ),
                              ),
                          ],
                        ),
                      ],
                      const SizedBox(height: 16),
                      Text(
                        '瞬时 ${_formatRate(state.instantBytesPerSecond)}  ·  '
                        '稳定 ${_formatRate(state.stableBytesPerSecond)}'
                        '${snapshot.lastFrameIndex == null ? '' : '  ·  帧 ${snapshot.lastFrameIndex}'}',
                      ),
                      if (state.lastError case final error?) ...[
                        const SizedBox(height: 10),
                        Text(error, style: const TextStyle(color: Color(0xffb42318))),
                      ],
                      if (state.isComplete) ...[
                        const SizedBox(height: 18),
                        SizedBox(
                          width: double.infinity,
                          child: FilledButton.icon(
                            onPressed: onShare,
                            icon: const Icon(Icons.ios_share),
                            label: const Text('保存或分享'),
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

Color _blockColor(MobileBlockKind kind) => switch (kind) {
  MobileBlockKind.missing => const Color(0xffb8b8b8),
  MobileBlockKind.partial => const Color(0xffffc247),
  MobileBlockKind.complete => const Color(0xff24945e),
  MobileBlockKind.failed => const Color(0xffd14343),
};

String _formatBytes(BigInt? bytes) {
  if (bytes == null) return '';
  final value = bytes.toDouble();
  if (value >= 1024 * 1024) return '${(value / 1024 / 1024).toStringAsFixed(1)} MB';
  if (value >= 1024) return '${(value / 1024).toStringAsFixed(1)} KB';
  return '$bytes B';
}

String _formatRate(double bytesPerSecond) {
  if (bytesPerSecond >= 1024 * 1024) {
    return '${(bytesPerSecond / 1024 / 1024).toStringAsFixed(1)} MB/s';
  }
  if (bytesPerSecond >= 1024) {
    return '${(bytesPerSecond / 1024).toStringAsFixed(1)} KB/s';
  }
  return '${bytesPerSecond.toStringAsFixed(0)} B/s';
}
