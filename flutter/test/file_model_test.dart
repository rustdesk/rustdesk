import 'dart:async';
import 'dart:convert';

import 'package:flutter_hbb/models/file_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

final _sessionId = UuidValue('00000000-0000-0000-0000-000000000000');

class _FakeFFI implements FFI {
  @override
  String id = 'test-peer';
  @override
  UuidValue get sessionId => _sessionId;
  @override
  late final FfiModel ffiModel = FfiModel(WeakReference(this));

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

FileController _createController(FileFetcher fileFetcher) {
  final ffi = _FakeFFI();
  return FileController(
    isLocal: false,
    getSessionID: () => _sessionId,
    rootState: WeakReference(ffi),
    jobController: JobController(() => _sessionId, () => null),
    fileFetcher: fileFetcher,
    getOtherSideDirectoryData: () =>
        DirectoryData(FileDirectory(), DirectoryOptions()),
  );
}

FileDirectory _directory(String path) => FileDirectory()..path = path;

String _directoryJson(String path) => jsonEncode({
      'id': 0,
      'path': path,
      'entries': <Object>[],
    });

class _SentRead {
  final String path;
  final bool includeHidden;

  const _SentRead(this.path, this.includeHidden);
}

void main() {
  test('a fast remote response is matched after registration', () async {
    late final FileFetcher fileFetcher;
    fileFetcher = FileFetcher(
      () => _sessionId,
      readRemoteDirectory: (_, path, __) {
        fileFetcher.tryCompleteTask(_directoryJson(path), 'false');
        return Future<void>.value();
      },
    );
    final directory = await fileFetcher.fetchDirectory('/fast', false, false);
    expect(directory.path, '/fast');
  });

  test('a send failure fails and removes its registered task', () async {
    final failure = StateError('send failed');
    final fileFetcher = FileFetcher(
      () => _sessionId,
      readRemoteDirectory: (_, __, ___) => Future<void>.error(failure),
    );
    await expectLater(
      fileFetcher.fetchDirectory('/failed', false, false),
      throwsA(same(failure)),
    );
    expect(fileFetcher.hasPendingRemoteRead('/failed'), isFalse);
  });

  test('a resolved Home path completes the sole empty-path request', () async {
    final sent = <_SentRead>[];
    final fileFetcher = FileFetcher(
      () => _sessionId,
      readRemoteDirectory: (_, path, includeHidden) async {
        sent.add(_SentRead(path, includeHidden));
      },
    );
    final controller = _createController(fileFetcher);
    controller.directory.value = _directory('/initial');

    final home = controller.openDirectory('');
    await Future<void>.delayed(Duration.zero);
    final response = _directoryJson('/home/user');
    controller.initDirAndHome({'value': response});
    expect(controller.homePath, '/home/user');
    expect(controller.directory.value.path, '/initial');
    fileFetcher.tryCompleteTask(response, 'false');
    expect(await home, isTrue);
    expect(controller.directory.value.path, '/home/user');
    expect(sent.single.path, isEmpty);
  });

  test('an automatic response initializes Home without a pending request', () {
    final controller = _createController(FileFetcher(() => _sessionId));

    controller.initDirAndHome({'value': _directoryJson('/home/user')});

    expect(controller.homePath, '/home/user');
    expect(controller.directory.value.path, '/home/user');
  });

  test('an exact path response is not taken by a pending Home request',
      () async {
    final sent = <_SentRead>[];
    final fileFetcher = FileFetcher(
      () => _sessionId,
      readRemoteDirectory: (_, path, includeHidden) async {
        sent.add(_SentRead(path, includeHidden));
      },
    );
    final home = fileFetcher.fetchDirectory('', false, false);
    final regular = fileFetcher.fetchDirectory('/regular', false, false);
    await Future<void>.delayed(Duration.zero);
    var homeCompleted = false;
    home.then<void>((_) => homeCompleted = true);

    fileFetcher.tryCompleteTask(_directoryJson('/unmatched'), 'false');
    await Future<void>.delayed(Duration.zero);
    expect(homeCompleted, isFalse);

    fileFetcher.tryCompleteTask(_directoryJson('/regular'), 'false');
    expect((await regular).path, '/regular');
    await Future<void>.delayed(Duration.zero);
    expect(homeCompleted, isFalse);

    fileFetcher.tryCompleteTask(_directoryJson('/home/user'), 'false');
    expect((await home).path, '/home/user');
    expect(sent.map((request) => request.path), ['', '/regular']);
  });

  test('a read error completes the sole pending request', () async {
    final fileFetcher = FileFetcher(
      () => _sessionId,
      readRemoteDirectory: (_, __, ___) async {},
    );
    final request = fileFetcher.fetchDirectory('/denied', false, false);
    await Future<void>.delayed(Duration.zero);
    final expectation = expectLater(request, throwsA('permission denied'));

    fileFetcher.tryCompleteRemoteTaskWithError('permission denied');

    await expectation;
    expect(fileFetcher.hasPendingRemoteRead('/denied'), isFalse);
  });

  test('same-path requests share the pending read', () async {
    final sent = <_SentRead>[];
    late final FileFetcher fileFetcher;
    fileFetcher = FileFetcher(
      () => _sessionId,
      readRemoteDirectory: (_, path, includeHidden) async {
        sent.add(_SentRead(path, includeHidden));
      },
    );
    final controller = _createController(fileFetcher);
    controller.directory.value = _directory('/initial');

    final first = controller.openDirectory('/same');
    final waiting = controller.openDirectory('/same');

    await Future<void>.delayed(Duration.zero);
    expect(sent.map((request) => request.path), ['/same']);
    fileFetcher.tryCompleteTask(_directoryJson('/same'), 'false');
    expect(await first, isTrue);
    await Future<void>.delayed(Duration.zero);

    expect(sent.map((request) => request.path), ['/same']);
    expect(await waiting, isTrue);
    expect(controller.directory.value.path, '/same');
  });

  test('same-path requests with different hidden options are serialized',
      () async {
    final sent = <_SentRead>[];
    final fileFetcher = FileFetcher(
      () => _sessionId,
      readRemoteDirectory: (_, path, includeHidden) async {
        sent.add(_SentRead(path, includeHidden));
      },
    );

    final first = fileFetcher.fetchDirectory('/same', false, false);
    final second = fileFetcher.fetchDirectory('/same', false, true);

    await Future<void>.delayed(Duration.zero);
    expect(sent.map((request) => request.includeHidden), [false]);

    fileFetcher.tryCompleteTask(_directoryJson('/same'), 'false');
    expect((await first).path, '/same');
    await Future<void>.delayed(Duration.zero);
    expect(sent.map((request) => request.includeHidden), [false, true]);

    fileFetcher.tryCompleteTask(_directoryJson('/same'), 'false');
    expect((await second).path, '/same');
  });

  test('session invalidation cancels active and waiting reads', () async {
    final sent = <_SentRead>[];
    final fileFetcher = FileFetcher(
      () => _sessionId,
      readRemoteDirectory: (_, path, includeHidden) async {
        sent.add(_SentRead(path, includeHidden));
      },
    );
    final first = fileFetcher.fetchDirectory('/same', false, false);
    final waiting = fileFetcher.fetchDirectory('/same', false, true);
    await Future<void>.delayed(Duration.zero);
    final firstError = expectLater(first, throwsA(isA<StateError>()));
    final waitingError = expectLater(waiting, throwsA(isA<StateError>()));

    fileFetcher.beginRemoteSession();

    await firstError;
    await Future<void>.delayed(Duration.zero);
    expect(sent.map((request) => request.includeHidden), [false]);
    await waitingError;
    expect(fileFetcher.hasPendingRemoteRead('/same'), isFalse);
    final replacement = fileFetcher.fetchDirectory('/same', false, true);
    await Future<void>.delayed(Duration.zero);
    expect(sent.map((request) => request.includeHidden), [false, true]);
    fileFetcher.tryCompleteTask(_directoryJson('/same'), 'false');
    expect((await replacement).path, '/same');
  });

  test('a late dispatch failure cannot remove a replacement task', () async {
    final dispatches = <Completer<void>>[];
    final fileFetcher = FileFetcher(
      () => _sessionId,
      readRemoteDirectory: (_, __, ___) {
        final dispatch = Completer<void>();
        dispatches.add(dispatch);
        return dispatch.future;
      },
    );
    final first = fileFetcher.fetchDirectory('/same', false, false);
    await Future<void>.delayed(Duration.zero);
    final firstError = expectLater(first, throwsA(isA<StateError>()));
    fileFetcher.beginRemoteSession();
    await firstError;

    final replacement = fileFetcher.fetchDirectory('/same', false, false);
    await Future<void>.delayed(Duration.zero);
    expect(dispatches, hasLength(2));
    dispatches.first.completeError(StateError('late dispatch failure'));
    await Future<void>.delayed(Duration.zero);

    expect(fileFetcher.hasPendingRemoteRead('/same'), isTrue);
    fileFetcher.tryCompleteTask(_directoryJson('/same'), 'false');
    expect((await replacement).path, '/same');
    dispatches.last.complete();
    await Future<void>.delayed(Duration.zero);
  });

  test('navigation ignores stale directory responses', () async {
    final fileFetcher = FileFetcher(
      () => _sessionId,
      readRemoteDirectory: (_, __, ___) async {},
    );
    final controller = _createController(fileFetcher);
    controller.directory.value = _directory('/initial');

    final stale = controller.openDirectory('/stale');
    final latest = controller.openDirectory('/latest');
    await Future<void>.delayed(Duration.zero);

    fileFetcher.tryCompleteTask(_directoryJson('/latest'), 'false');
    expect(await latest, isTrue);
    fileFetcher.tryCompleteTask(_directoryJson('/stale'), 'false');
    expect(await stale, isTrue);

    expect(controller.directory.value.path, '/latest');
  });
}
