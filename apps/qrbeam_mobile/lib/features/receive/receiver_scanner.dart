import 'package:flutter_zxing/flutter_zxing.dart';

ReaderWidget buildReceiverScanner({required void Function(Code) onScan}) =>
    ReaderWidget(
      onScan: onScan,
      codeFormat: Format.qrCode,
      maxNumberOfSymbols: 1,
      tryHarder: true,
      tryRotate: true,
      tryDownscale: true,
      resolution: ResolutionPreset.veryHigh,
      showFlashlight: false,
      showToggleCamera: false,
      showGallery: false,
      scanDelay: Duration.zero,
      scanDelaySuccess: Duration.zero,
      cropPercent: 0.82,
    );
