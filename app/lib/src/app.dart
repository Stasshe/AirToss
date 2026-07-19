import 'package:airtoss/src/discovery/screen.dart';
import 'package:flutter/material.dart';

class AirTossApp extends StatelessWidget {
  const AirTossApp({super.key, this.home});

  final Widget? home;

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.fromSeed(
      seedColor: const Color(0xff006d77),
      brightness: Brightness.light,
    );

    return MaterialApp(
      title: 'AirToss',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: colorScheme,
        scaffoldBackgroundColor: const Color(0xfff7f9f9),
        useMaterial3: true,
      ),
      home: home ?? const DiscoveryScreen(peers: []),
    );
  }
}
