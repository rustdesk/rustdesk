import 'dart:async';
import 'dart:io';
import 'dart:ui';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:desktop_multi_window/experimental_windowing.dart';
import 'package:flutter/widgets.dart';

void main() {
  final handlersReady = <int>{};
  final backend = ExperimentalWindowingBackend.install(
    keepImplicitMainView: Platform.isLinux,
    builder: (window) {
      if (handlersReady.add(window.windowId)) {
        window.run(() {
          DesktopMultiWindow.setMethodHandler((call, from) async {
            if (call.method == 'write') window.state['value'] = call.arguments;
            return [from, window.arguments, window.state['value']];
          });
        });
      }
      return Directionality(
        textDirection: TextDirection.ltr,
        child: Text('Window ${window.windowId}'),
      );
    },
  );
  runWidget(backend.root);
  WidgetsBinding.instance.addPostFrameCallback((_) async {
    try {
      await checkWindows(backend, handlersReady);
      stdout.writeln('Native windows opened, routed messages, and closed.');
      exit(0);
    } catch (error, stack) {
      stderr.writeln('$error\n$stack');
      exit(1);
    }
  });
}

Future<void> waitFor(
  String description,
  FutureOr<bool> Function() ready,
) async {
  final timeout = Stopwatch()..start();
  while (!await ready()) {
    if (timeout.elapsed > const Duration(seconds: 10)) {
      throw StateError('Timed out waiting for $description');
    }
    await Future<void>.delayed(const Duration(milliseconds: 50));
  }
}

Future<void> checkWindows(
  ExperimentalWindowingBackend backend,
  Set<int> handlersReady,
) async {
  final dispatcher = PlatformDispatcher.instance;
  final initialViews = dispatcher.views.length;
  final mainViewId = backend.viewIdFor(0);
  if (!dispatcher.views.any((view) => view.viewId == mainViewId)) {
    throw StateError('The main Flutter view is missing');
  }
  final first = await DesktopMultiWindow.createWindow('first');
  final second = await DesktopMultiWindow.createWindow('second');
  await first.show();
  await second.show();
  await waitFor(
    'both Flutter views to mount',
    () =>
        dispatcher.views.length == initialViews + 2 &&
        handlersReady.containsAll([first.windowId, second.windowId]),
  );
  // The first show request presents the native window after its first frame.
  // Mounted Dart handlers can be ready before either window has been presented.
  await waitFor(
    'both native windows to become visible',
    () async => !await first.isHidden() && !await second.isHidden(),
  );

  final reply =
      await backend
              .contextFor(second.windowId)
              .run(
                () => DesktopMultiWindow.invokeMethod(
                  first.windowId,
                  'write',
                  42,
                ),
              )
          as List<Object?>;
  final sibling = await DesktopMultiWindow.invokeMethod(
    second.windowId,
    'read',
  ) as List<Object?>;
  if (reply[0] != second.windowId ||
      reply[1] != 'first' ||
      reply[2] != 42 ||
      sibling[0] != 0 ||
      sibling[1] != 'second' ||
      sibling[2] != null) {
    throw StateError('Window state or message routing leaked');
  }

  await first.hide();
  await waitFor('the first native window to hide', first.isHidden);
  if (await second.isHidden()) {
    throw StateError('Hiding the first window also hid its sibling');
  }
  await first.show();
  await waitFor(
    'the first native window to reopen',
    () async => !await first.isHidden(),
  );
  final frame = await first.getFrame();
  if (frame.isEmpty) {
    throw StateError('Native window did not reopen: $frame');
  }
  await first.close();
  await waitFor(
    'the first Flutter view to close',
    () => dispatcher.views.length == initialViews + 1,
  );
  await second.close();
  await waitFor(
    'the second Flutter view to close',
    () =>
        dispatcher.views.length == initialViews &&
        dispatcher.views.any((view) => view.viewId == mainViewId),
  );
}
