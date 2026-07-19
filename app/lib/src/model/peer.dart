import 'package:flutter/widgets.dart';

enum PeerPlatform { ios, android, windows, macos, linux }

enum RouteEstimate {
  fast('高速接続が利用できます'),
  wifiSwitch('高速接続には Wi-Fi の切り替えが必要です'),
  bluetoothOnly('Bluetooth での低速転送のみ利用できます');

  const RouteEstimate(this.label);

  final String label;
}

@immutable
class PeerView {
  const PeerView({
    required this.sessionDeviceId,
    required this.name,
    required this.platform,
    required this.routeEstimate,
  });

  final String sessionDeviceId;
  final String name;
  final PeerPlatform platform;
  final RouteEstimate routeEstimate;
}
