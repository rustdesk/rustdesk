import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:desktop_multi_window/experimental_windowing.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';

late ExperimentalWindowingBackend backend;
int sharedGlobal = 0;
final results = <Map<String, Object?>>[];

void main(List<String> args) {
  final keepImplicitMainView = args.contains('--implicit-main-self-test');
  backend = ExperimentalWindowingBackend.install(
    keepImplicitMainView: keepImplicitMainView,
    builder: (window) => MaterialApp(
      key: ValueKey(window.windowId),
      home: WindowPage(window: window),
    ),
  );
  runWidget(backend.root);
  if (const bool.fromEnvironment('POC_SELF_TEST')) {
    WidgetsBinding.instance.addPostFrameCallback(
      (_) => unawaited(runChecks(keepImplicitMainView: keepImplicitMainView)),
    );
  }
}

class WindowPage extends StatefulWidget {
  const WindowPage({super.key, required this.window});
  final ExperimentalWindowContext window;
  @override
  State<WindowPage> createState() => _WindowPageState();
}

class _WindowPageState extends State<WindowPage> {
  int counter = 0;
  String message = '';
  late final TextEditingController text;
  ExperimentalWindowContext get window => widget.window;
  @override
  void initState() {
    super.initState();
    text = TextEditingController();
    window.run(
      () => DesktopMultiWindow.setMethodHandler((call, from) async {
        await Future<void>.delayed(const Duration(milliseconds: 10));
        switch (call.method) {
          case 'increment':
            setState(() => counter++);
            break;
          case 'setText':
            text.text = call.arguments as String;
            break;
          case 'mutate':
            (call.arguments as List<dynamic>)[0] = 'target-only';
            break;
          case 'global':
            sharedGlobal = call.arguments as int;
            break;
          case 'fail':
            throw PlatformException(
              code: 'intentional',
              message: 'test failure',
            );
        }
        setState(() => message = '${call.method} from $from');
        return <String, Object?>{
          'id': window.windowId,
          'zone': backend.currentWindowId,
          'from': from,
          'counter': counter,
          'text': text.text,
          'global': sharedGlobal,
          'arguments': call.arguments,
        };
      }),
    );
  }

  @override
  void dispose() {
    text.dispose();
    super.dispose();
  }

  Future<void> act(Future<void> Function() action) async {
    try {
      await window.run(action);
    } catch (error) {
      if (mounted) setState(() => message = '$error');
    }
  }

  @override
  Widget build(BuildContext context) => Scaffold(
    appBar: AppBar(title: Text('Official backend — window ${window.windowId}')),
    body: Padding(
      padding: const EdgeInsets.all(20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Independent widget counter: $counter'),
          Text('Arguments: ${window.arguments ?? "main"}'),
          TextField(
            controller: text,
            decoration: const InputDecoration(labelText: 'Per-window text'),
          ),
          const SizedBox(height: 12),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              FilledButton(
                onPressed: () => act(() async {
                  final child = await DesktopMultiWindow.createWindow(
                    'from ${window.windowId}',
                  );
                  await child.setTitle('PoC child ${child.windowId}');
                  await child.show();
                }),
                child: const Text('New window'),
              ),
              OutlinedButton(
                onPressed: () => act(() async {
                  await DesktopMultiWindow.invokeMethod(
                    window.windowId,
                    'increment',
                  );
                }),
                child: const Text('Increment here'),
              ),
              OutlinedButton(
                onPressed: () => act(() async {
                  final reply = await DesktopMultiWindow.invokeMethod(
                    0,
                    'inspect',
                  );
                  if (mounted) setState(() => message = jsonEncode(reply));
                }),
                child: const Text('Message main'),
              ),
              OutlinedButton(
                onPressed: () => act(
                  () =>
                      WindowController.fromWindowId(window.windowId).maximize(),
                ),
                child: const Text('Maximize'),
              ),
              OutlinedButton(
                onPressed: () => act(
                  () =>
                      WindowController.fromWindowId(window.windowId)
                          .unmaximize(),
                ),
                child: const Text('Restore'),
              ),
              OutlinedButton(
                onPressed: () => act(
                  () =>
                      WindowController.fromWindowId(window.windowId)
                          .setFullscreen(true),
                ),
                child: const Text('Fullscreen'),
              ),
              OutlinedButton(
                onPressed: () => act(
                  () =>
                      WindowController.fromWindowId(window.windowId)
                          .setFullscreen(false),
                ),
                child: const Text('Leave fullscreen'),
              ),
              OutlinedButton(
                onPressed: () => act(
                  () => WindowController.fromWindowId(window.windowId).close(),
                ),
                child: const Text('Close'),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Text(message),
          const Text(
            'Single engine / shared Dart isolate. Globals are NOT isolated.',
          ),
        ],
      ),
    ),
  );
}

void check(String name, bool ok, [Object? detail]) {
  results.add({'name': name, 'pass': ok, 'detail': detail?.toString()});
  stdout.writeln('POC ${ok ? "PASS" : "FAIL"} $name ${detail ?? ""}');
  if (!ok) throw StateError(name);
}

Future<void> poll(String name, FutureOr<bool> Function() condition) async {
  for (var i = 0; i < 100; i++) {
    if (await condition()) {
      check(name, true);
      return;
    }
    await Future<void>.delayed(const Duration(milliseconds: 50));
  }
  check(name, false);
}

Future<void> rejected(
  String name,
  Future<dynamic> Function() action,
  bool Function(Object) expected,
) async {
  try {
    await action();
  } catch (error) {
    check(name, expected(error), error);
    return;
  }
  check(name, false, 'unexpected success');
}

class CloseListener extends MultiWindowListener {
  int closes = 0;
  int moves = 0;
  int resizes = 0;
  @override
  void onWindowClose() => closes++;
  @override
  void onWindowMoved() => moves++;
  @override
  void onWindowResized() => resizes++;
}

Future<List<dynamic>> nativeProbe([String? title, bool close = false]) async {
  final result = await Process.run('powershell.exe', [
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-File',
    'tool/native_probe.ps1',
    '-AppProcessId',
    '$pid',
    if (title != null) ...['-Title', title],
    if (close) '-RequestClose',
  ]);
  if (result.exitCode != 0) throw StateError('${result.stderr}');
  return jsonDecode(result.stdout as String) as List<dynamic>;
}

Future<void> runChecks({required bool keepImplicitMainView}) async {
  var code = 0;
  try {
    await Future<void>.delayed(const Duration(milliseconds: 800));
    final initialViews = ui.PlatformDispatcher.instance.views.length;
    check('main official view mounted', initialViews >= 1);
    if (keepImplicitMainView) {
      final implicitView = ui.PlatformDispatcher.instance.implicitView;
      check(
        'existing main FlutterView retained',
        implicitView != null && backend.viewIdFor(0) == implicitView.viewId,
      );
    }
    final a = await DesktopMultiWindow.createWindow('remote');
    final b = await DesktopMultiWindow.createWindow('file-transfer');
    check(
      'unique IDs and staged windows',
      a.windowId > 0 &&
          b.windowId > a.windowId &&
          ui.PlatformDispatcher.instance.views.length == initialViews,
    );
    await a.setTitle('PoC test A');
    const initialFrame = Rect.fromLTWH(60, 100, 570, 390);
    await a.setFrame(initialFrame);
    await a.showTitleBar(false);
    check('staged controls keep A hidden', await a.isHidden());
    await b.setTitle('PoC test B');
    await a.show();
    await b.show();
    await poll(
      'two additional native views created',
      () => ui.PlatformDispatcher.instance.views.length == initialViews + 2,
    );
    await Future<void>.delayed(const Duration(milliseconds: 500));
    check(
      'staged native frame applied before showing',
      await a.getFrame() == initialFrame,
    );
    check(
      'staged caption hidden on first show',
      (await nativeProbe('PoC test A')).single['caption'] == false,
    );
    await a.startDragging();
    final dragWindows = await nativeProbe();
    check(
      'captionless drag posts the native move command to A only',
      dragWindows
              .where((window) => window['moveCommand'] != 0)
              .singleOrNull?['title'] ==
          'PoC test A',
    );
    await a.showTitleBar(true);
    check(
      'distinct FlutterView IDs',
      {
            backend.viewIdFor(0),
            backend.viewIdFor(a.windowId),
            backend.viewIdFor(b.windowId),
          }.length ==
          3,
    );
    final nativeWindows = await nativeProbe();
    check(
      'three visible native HWNDs (no extra bootstrap window)',
      nativeWindows.where((w) => w['visible'] == true).length == 3 &&
          nativeWindows.every((w) => w['title'] != 'windowing_poc'),
      jsonEncode(nativeWindows),
    );
    check(
      'native titles applied',
      (await nativeProbe('PoC test A')).length == 1 &&
          (await nativeProbe('PoC test B')).length == 1,
    );
    check(
      'ID enumeration',
      (await DesktopMultiWindow.getAllSubWindowIds()).join(',') ==
          '${a.windowId},${b.windowId}',
    );
    final reply =
        await DesktopMultiWindow.invokeMethod(a.windowId, 'increment') as Map;
    final other =
        await DesktopMultiWindow.invokeMethod(b.windowId, 'inspect') as Map;
    check(
      'widget state isolated',
      reply['counter'] == 1 && other['counter'] == 0,
    );
    await DesktopMultiWindow.invokeMethod(a.windowId, 'setText', 'remote-only');
    final textB =
        await DesktopMultiWindow.invokeMethod(b.windowId, 'inspect') as Map;
    check('text controller isolated', textB['text'] == '');
    final cross =
        await backend
                .contextFor(a.windowId)
                .run(
                  () => DesktopMultiWindow.invokeMethod(b.windowId, 'inspect'),
                )
            as Map;
    check(
      'sender ID and target zone after await',
      cross['from'] == a.windowId &&
          cross['id'] == b.windowId &&
          cross['zone'] == b.windowId,
    );
    final payload = <String>['source'];
    final copied = await DesktopMultiWindow.invokeMethod(
      a.windowId,
      'mutate',
      payload,
    ) as Map;
    check(
      'codec copy prevents aliasing',
      payload.single == 'source' &&
          (copied['arguments'] as List).single == 'target-only',
    );
    await DesktopMultiWindow.invokeMethod(a.windowId, 'global', 73);
    final globalB =
        await DesktopMultiWindow.invokeMethod(b.windowId, 'inspect') as Map;
    check(
      'global sharing detected (migration blocker)',
      globalB['global'] == 73,
    );
    await rejected(
      'handler errors propagated',
      () => DesktopMultiWindow.invokeMethod(a.windowId, 'fail'),
      (e) => e is PlatformException && e.code == 'intentional',
    );
    await a.maximize();
    await poll('Windows maximize observed', a.isMaximized);
    check(
      'Win32 IsZoomed confirms maximize',
      (await nativeProbe('PoC test A')).single['maximized'] == true,
    );
    check('maximize isolated from B', !await b.isMaximized());
    await a.unmaximize();
    await poll(
      'Windows unmaximize observed',
      () async => !await a.isMaximized(),
    );
    await a.minimize();
    await poll('Windows minimize observed', a.isMinimized);
    check(
      'Win32 IsIconic confirms minimize',
      (await nativeProbe('PoC test A')).single['minimized'] == true,
    );
    await a.show();
    await poll(
      'Windows show restores minimized',
      () async => !await a.isMinimized(),
    );
    await a.focus();
    final focused =
        (await nativeProbe('PoC test A')).single['foreground'] == true;
    results.add({
      'name': 'focus foreground parity',
      'pass': null,
      'detail':
          'Win32 foreground=$focused after SetForegroundWindow; Windows may deny activation',
    });
    stdout.writeln('POC OBSERVATION focus foreground=$focused');
    await a.setFullscreen(true);
    await poll('Windows fullscreen observed', a.isFullScreen);
    await a.setFullscreen(false);
    await poll(
      'Windows fullscreen restored',
      () async => !await a.isFullScreen(),
    );
    await a.hide();
    await poll(
      'hide changes native visibility',
      () async =>
          await a.isHidden() &&
          (await nativeProbe('PoC test A')).single['visible'] == false,
    );
    check('hide leaves B visible', !await b.isHidden());
    final listener = CloseListener();
    final siblingListener = CloseListener();
    backend
        .contextFor(a.windowId)
        .run(() => DesktopMultiWindow.addListener(listener));
    backend
        .contextFor(b.windowId)
        .run(() => DesktopMultiWindow.addListener(siblingListener));
    const frame = Rect.fromLTWH(75, 100, 530, 365);
    await a.setFrame(frame);
    await poll(
      'native frame change reaches its window listener',
      () => listener.moves > 0 && listener.resizes > 0,
    );
    check(
      'native frame events stay out of the sibling window',
      siblingListener.moves == 0 && siblingListener.resizes == 0,
    );
    check('setFrame preserves hidden state', await a.isHidden());
    check('Win32 outer frame round trips', await a.getFrame() == frame);
    await a.show();
    await poll(
      'show restores hidden HWND',
      () async =>
          !await a.isHidden() &&
          (await nativeProbe('PoC test A')).single['visible'] == true,
    );
    check('show preserves outer frame', await a.getFrame() == frame);
    await a.center();
    final centered = await a.getFrame();
    check(
      'center moves window without resizing',
      centered.topLeft != frame.topLeft && centered.size == frame.size,
    );
    await a.showTitleBar(false);
    check(
      'Win32 caption style removed',
      (await nativeProbe('PoC test A')).single['caption'] == false,
    );
    await a.showTitleBar(true);
    check(
      'Win32 caption style restored',
      (await nativeProbe('PoC test A')).single['caption'] == true,
    );
    check(
      'title bar toggling leaves B unchanged',
      (await nativeProbe('PoC test B')).single['caption'] == true,
    );
    await a.setPreventClose(true);
    await a.close();
    check(
      'prevent close and emit event',
      listener.closes == 1 &&
          (await DesktopMultiWindow.getAllSubWindowIds()).contains(a.windowId),
    );
    await nativeProbe('PoC test A', true);
    await poll('native WM_CLOSE intercepted', () => listener.closes == 2);
    check(
      'intercepted HWND still exists',
      (await nativeProbe('PoC test A')).length == 1,
    );
    await a.setPreventClose(false);
    await nativeProbe('PoC test A', true);
    await poll(
      'native view destroyed',
      () => ui.PlatformDispatcher.instance.views.length == initialViews + 1,
    );
    await rejected(
      'closed ID rejected',
      () => DesktopMultiWindow.invokeMethod(a.windowId, 'inspect'),
      (e) => e is StateError,
    );
    check('closed HWND removed', (await nativeProbe('PoC test A')).isEmpty);
    await b.close();
    await poll(
      'all child views destroyed',
      () => ui.PlatformDispatcher.instance.views.length == initialViews,
    );
    final c = await DesktopMultiWindow.createWindow('recreated');
    check('IDs never reused', c.windowId > b.windowId);
    await c.close();
    check(
      'staged close cleaned up',
      (await DesktopMultiWindow.getAllSubWindowIds()).isEmpty,
    );
  } catch (error, stack) {
    code = 1;
    results.add({
      'name': 'exception',
      'pass': false,
      'detail': '$error\n$stack',
    });
    stderr.writeln('$error\n$stack');
  } finally {
    final file = File(
      Platform.environment['POC_RESULTS'] ?? 'poc-results.json',
    );
    await file.writeAsString(
      const JsonEncoder.withIndent('  ')
          .convert({'exitCode': code, 'results': results}),
    );
    exit(code);
  }
}
