import 'package:flutter/material.dart';
import 'package:qrbeam_mobile/features/receive/receive_page.dart';
import 'package:qrbeam_mobile/src/rust/frb_generated.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'QRBeam',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xff177245)),
        useMaterial3: true,
      ),
      home: const ReceivePage(),
    );
  }
}
