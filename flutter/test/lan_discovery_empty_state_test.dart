import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/peers_view.dart';
import 'package:flutter_hbb/models/peer_model.dart';

void main() {
  Widget app(
    DiscoveryState state,
    VoidCallback onRetry, {
    bool foundPeers = false,
    bool firewallBlocked = false,
    bool discoveryEnabled = true,
    Widget? child,
  }) {
    return MaterialApp(
      home: LanDiscoveryResultView(
        state: state,
        foundPeers: foundPeers,
        firewallBlocked: firewallBlocked,
        discoveryEnabled: discoveryEnabled,
        onRetry: onRetry,
        child: child ??
            LanDiscoveryEmptyState(
              state: state,
              translateText: (text) => text,
            ),
        translateText: (text) => text,
      ),
    );
  }

  testWidgets('shows progress only while discovery is scanning',
      (tester) async {
    await tester.pumpWidget(app(DiscoveryState.scanning, () {}));

    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    expect(find.text('Waiting'), findsOneWidget);
    expect(find.text('Retry'), findsNothing);
  });

  testWidgets('shows guidance and retries after discovery completes',
      (tester) async {
    var retries = 0;
    await tester.pumpWidget(app(DiscoveryState.completed, () => retries++));

    expect(
      find.text(
        'No devices responded. Make sure RustDesk is running on other devices and allowed to answer LAN discovery requests. A firewall or network isolation can prevent replies.',
      ),
      findsOneWidget,
    );
    await tester.ensureVisible(find.text('Retry'));
    await tester.tap(find.text('Retry'));

    expect(retries, 1);
  });

  testWidgets('keeps cached peers visible when a scan gets no responses',
      (tester) async {
    var retries = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: LanDiscoveryResultView(
          state: DiscoveryState.completed,
          foundPeers: false,
          firewallBlocked: false,
          discoveryEnabled: true,
          onRetry: () => retries++,
          translateText: (text) => text,
          child: const Text('Cached peer'),
        ),
      ),
    );

    expect(find.text('Cached peer'), findsOneWidget);
    expect(
      find.text(
        'No devices responded. Make sure RustDesk is running on other devices and allowed to answer LAN discovery requests. A firewall or network isolation can prevent replies.',
      ),
      findsOneWidget,
    );
    await tester.tap(find.text('Retry'));

    expect(retries, 1);
  });

  testWidgets('reports a failed scan without claiming devices did not respond',
      (tester) async {
    await tester.pumpWidget(app(DiscoveryState.failed, () {}));

    expect(find.text('Failed'), findsOneWidget);
    expect(find.textContaining('No devices responded'), findsNothing);
    expect(find.text('Retry'), findsOneWidget);
  });

  testWidgets('lays out the cached peer banner at narrow width',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(320, 600));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(
      MaterialApp(
        home: LanDiscoveryResultView(
          state: DiscoveryState.completed,
          foundPeers: false,
          firewallBlocked: false,
          discoveryEnabled: true,
          onRetry: () {},
          translateText: (text) => text,
          child: const Text('Cached peer'),
        ),
      ),
    );

    expect(tester.takeException(), isNull);
    expect(find.text('Cached peer'), findsOneWidget);
    expect(find.text('Retry'), findsOneWidget);
  });

  testWidgets('shows a firewall warning beside visible peers', (tester) async {
    await tester.pumpWidget(app(
      DiscoveryState.completed,
      () {},
      foundPeers: true,
      firewallBlocked: true,
      child: const Text('Tailnet peer'),
    ));

    expect(find.text('Tailnet peer'), findsOneWidget);
    expect(
      find.text(
        "This device's firewall blocked LAN discovery. Some available devices may not appear in this list.",
      ),
      findsOneWidget,
    );
    expect(find.textContaining('No devices responded'), findsNothing);
  });

  testWidgets('explains disabled incoming discovery beside visible peers',
      (tester) async {
    await tester.pumpWidget(app(
      DiscoveryState.completed,
      () {},
      foundPeers: true,
      discoveryEnabled: false,
      child: const Text('Tailnet peer'),
    ));

    expect(find.text('Tailnet peer'), findsOneWidget);
    expect(
      find.text(
        'This device will not answer LAN discovery requests because "Deny LAN discovery" is enabled. It can still discover other devices.',
      ),
      findsOneWidget,
    );
    expect(find.text('Retry'), findsNothing);
  });
}
