import 'package:qrbeam_mobile/src/rust/api/receiver.dart';

class ReceiveViewModel {
  const ReceiveViewModel({
    required this.snapshot,
    this.instantBytesPerSecond = 0,
    this.stableBytesPerSecond = 0,
    this.lastError,
  });

  factory ReceiveViewModel.fromSnapshot(
    MobileSnapshot snapshot, {
    double instantBytesPerSecond = 0,
    double stableBytesPerSecond = 0,
    String? lastError,
  }) => ReceiveViewModel(
    snapshot: snapshot,
    instantBytesPerSecond: instantBytesPerSecond,
    stableBytesPerSecond: stableBytesPerSecond,
    lastError: lastError,
  );

  final MobileSnapshot snapshot;
  final double instantBytesPerSecond;
  final double stableBytesPerSecond;
  final String? lastError;

  bool get isComplete => snapshot.phase == MobilePhase.complete;

  double get progress {
    if (isComplete) return 1;
    if (snapshot.blocks.isEmpty) return 0;
    var total = 0.0;
    for (final block in snapshot.blocks) {
      total += switch (block.kind) {
        MobileBlockKind.complete => 1,
        MobileBlockKind.partial when block.required_ > 0 =>
          (block.unique / block.required_).clamp(0, 1),
        MobileBlockKind.missing ||
        MobileBlockKind.partial ||
        MobileBlockKind.failed => 0,
      };
    }
    return (total / snapshot.blocks.length).clamp(0, 0.99);
  }

  String get title => switch (snapshot.phase) {
    MobilePhase.waitingManifest => '等待文件清单',
    MobilePhase.receiving => '正在接收',
    MobilePhase.complete => '接收完成',
  };
}

class ThroughputWindow {
  final List<_Sample> _samples = [];
  DateTime? _lastAt;
  double instantBytesPerSecond = 0;

  void record(int bytes, DateTime at) {
    final previous = _lastAt;
    if (previous != null) {
      final elapsed = at.difference(previous).inMicroseconds / 1000000;
      if (elapsed > 0) instantBytesPerSecond = bytes / elapsed;
    }
    _lastAt = at;
    _samples.add(_Sample(bytes, at));
    _prune(at);
  }

  double stableBytesPerSecond(DateTime now) {
    _prune(now);
    if (_samples.isEmpty) return 0;
    final elapsed = now.difference(_samples.first.at).inMicroseconds / 1000000;
    if (elapsed <= 0) return 0;
    final totalBytes = _samples.fold<int>(0, (sum, sample) => sum + sample.bytes);
    return totalBytes / elapsed;
  }

  void _prune(DateTime now) {
    final cutoff = now.subtract(const Duration(seconds: 10));
    _samples.removeWhere((sample) => sample.at.isBefore(cutoff));
  }
}

class _Sample {
  const _Sample(this.bytes, this.at);

  final int bytes;
  final DateTime at;
}
