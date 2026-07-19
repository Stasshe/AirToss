import 'package:airtoss/src/app.dart';
import 'package:airtoss/src/discovery/screen.dart';
import 'package:airtoss/src/model/peer.dart';
import 'package:airtoss/src/verification/screen.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('discovery shows peer details and selects the peer', (
    tester,
  ) async {
    const peer = PeerView(
      sessionDeviceId: '0102030405060708',
      name: 'Ubuntu desktop',
      platform: PeerPlatform.linux,
      routeEstimate: RouteEstimate.fast,
    );
    PeerView? selectedPeer;

    await tester.pumpWidget(
      AirTossApp(
        home: DiscoveryScreen(
          peers: const [peer],
          isScanning: true,
          onPeerSelected: (peer) => selectedPeer = peer,
        ),
      ),
    );

    expect(find.text('Bluetooth で探しています'), findsOneWidget);
    expect(find.text('Ubuntu desktop'), findsOneWidget);
    expect(find.text('Bluetooth で発見'), findsOneWidget);
    expect(find.text('高速接続が利用できます'), findsOneWidget);

    await tester.tap(find.text('Ubuntu desktop'));

    expect(selectedPeer, same(peer));
  });

  testWidgets('verification exposes both user decisions', (tester) async {
    var confirmed = false;
    var cancelled = false;

    await tester.pumpWidget(
      AirTossApp(
        home: VerificationScreen(
          code: '048217',
          peerName: 'Kentaro’s iPhone',
          onConfirmed: () => confirmed = true,
          onCancelled: () => cancelled = true,
        ),
      ),
    );

    expect(find.text('0  4  8  2  1  7'), findsOneWidget);

    await tester.tap(find.text('一致している'));
    await tester.tap(find.text('キャンセル'));

    expect(confirmed, isTrue);
    expect(cancelled, isTrue);
  });

  testWidgets('discovery failure offers retry', (tester) async {
    var retried = false;

    await tester.pumpWidget(
      AirTossApp(
        home: DiscoveryScreen(
          peers: const [],
          failureMessage: 'Bluetooth を開始できませんでした',
          onRetry: () => retried = true,
        ),
      ),
    );

    expect(find.text('Bluetooth を開始できませんでした'), findsOneWidget);

    await tester.tap(find.text('再試行'));

    expect(retried, isTrue);
  });
}
