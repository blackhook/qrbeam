import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_zxing/flutter_zxing.dart';
import 'package:qrbeam_mobile/features/receive/frame_ingest_gate.dart';
import 'package:qrbeam_mobile/features/receive/receiver_scanner.dart';
import 'package:qrbeam_mobile/features/receive/receive_screen.dart';
import 'package:qrbeam_mobile/features/receive/receive_view_model.dart';
import 'package:qrbeam_mobile/src/rust/api/receiver.dart';
import 'package:share_plus/share_plus.dart';

class ReceivePage extends StatefulWidget {
  const ReceivePage({super.key});

  @override
  State<ReceivePage> createState() => _ReceivePageState();
}

class _ReceivePageState extends State<ReceivePage> {
  static const _waiting = MobileSnapshot(
    phase: MobilePhase.waitingManifest,
    blocks: [],
  );

  final ThroughputWindow _throughput = ThroughputWindow();
  late final FrameIngestGate _gate = FrameIngestGate(_ingestFrame);
  MobileReceiver? _receiver;
  MobileSnapshot _snapshot = _waiting;
  String? _lastError;

  @override
  void initState() {
    super.initState();
    unawaited(_initializeReceiver());
  }

  Future<void> _initializeReceiver() async {
    final receiver = await MobileReceiver.newInstance();
    if (!mounted) {
      receiver.dispose();
      return;
    }
    _receiver = receiver;
    setState(() {});
  }

  Future<void> _ingestFrame(List<int> frame) async {
    final receiver = _receiver;
    if (receiver == null || frame.isEmpty) return;
    MobileSnapshot next;
    String? error;
    try {
      next = await receiver.ingest(frame: frame);
      _throughput.record(frame.length, DateTime.now());
    } catch (exception) {
      next = await receiver.snapshot();
      error = _friendlyError(exception);
    }
    if (!mounted) return;
    setState(() {
      _snapshot = next;
      _lastError = error;
    });
  }

  void _onScan(Code code) {
    final bytes = code.rawBytes;
    if (!code.isValid || bytes == null || bytes.isEmpty) return;
    unawaited(_gate.submit(bytes));
  }

  Future<void> _share() async {
    final receiver = _receiver;
    if (receiver == null) return;
    final box = context.findRenderObject() as RenderBox?;
    final sharePositionOrigin = box == null
        ? null
        : box.localToGlobal(Offset.zero) & box.size;
    final bytes = await receiver.completedFile();
    if (bytes == null) return;
    final filename = _snapshot.filename ?? 'received.bin';
    await SharePlus.instance.share(
      ShareParams(
        files: [XFile.fromData(bytes)],
        fileNameOverrides: [filename],
        sharePositionOrigin: sharePositionOrigin,
      ),
    );
  }

  @override
  void dispose() {
    _receiver?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final now = DateTime.now();
    return ReceiveScreen(
      state: ReceiveViewModel.fromSnapshot(
        _snapshot,
        instantBytesPerSecond: _throughput.instantBytesPerSecond,
        stableBytesPerSecond: _throughput.stableBytesPerSecond(now),
        lastError: _lastError,
      ),
      scanner: buildReceiverScanner(
        onScan: _onScan,
      ),
      onShare: _share,
    );
  }
}

String _friendlyError(Object exception) {
  final message = exception.toString();
  if (message.contains('CRC32C')) {
    return '区块校验失败，请在电脑上回补红色区块';
  }
  if (message.contains('manifest')) return '文件清单不完整，继续对准二维码';
  return '这一帧没有通过校验，已忽略并继续接收';
}
