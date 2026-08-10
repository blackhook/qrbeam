typedef FrameHandler = Future<void> Function(List<int> frame);

class FrameIngestGate {
  FrameIngestGate(this._handler);

  final FrameHandler _handler;
  bool _busy = false;

  bool get isBusy => _busy;

  Future<bool> submit(List<int> frame) async {
    if (_busy) return false;
    _busy = true;
    try {
      await _handler(frame);
      return true;
    } finally {
      _busy = false;
    }
  }
}
