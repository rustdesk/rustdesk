import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/shortcut_model.dart';

class _Session extends Fake implements FFI {
  @override
  bool closed = false;
}

void main() {
  late _Session session;
  late ShortcutModel model;

  setUp(() {
    session = _Session();
    model = ShortcutModel(WeakReference(session));
  });

  tearDown(() => model.clear());

  test('a removed menu action cannot run its cached callback', () async {
    var calls = 0;
    model.register('mute', () => calls++);
    model.registerRefresher(['mute'], () => model.unregister('mute'));

    model.onTriggered('mute');
    await pumpEventQueue();

    expect(calls, 0);
  });

  test('an action becomes available without reopening its menu', () async {
    var calls = 0;
    model.registerRefresher(['screenshot'], () {
      model.register('screenshot', () => calls++);
    });

    model.onTriggered('screenshot');
    await pumpEventQueue();

    expect(calls, 1);
  });

  test('refresh finishes before invoking the current callback', () async {
    final ready = Completer<void>();
    final calls = <String>[];
    model.register('cursor', () => calls.add('stale'));
    model.registerRefresher(['cursor'], () async {
      await ready.future;
      model.register('cursor', () => calls.add('current'));
    });

    model.onTriggered('cursor');
    expect(calls, isEmpty);
    ready.complete();
    await pumpEventQueue();

    expect(calls, ['current']);
  });

  test('closing during a refresh prevents late registration and dispatch',
      () async {
    final ready = Completer<void>();
    var calls = 0;
    model.registerRefresher(['mute'], () async {
      await ready.future;
      model.register('mute', () => calls++);
    });

    model.onTriggered('mute');
    session.closed = true;
    model.clear();
    ready.complete();
    await pumpEventQueue();
    model.onTriggered('mute');
    await pumpEventQueue();

    expect(calls, 0);
  });
}
