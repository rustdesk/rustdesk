import 'dart:async';
import 'dart:convert';

import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/file_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
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

class _FileBinding extends Fake implements RustdeskImpl {
  int _jobId = 0;
  final emptyDirsDispatch = Completer<void>();

  @override
  Future<void> sessionReadRemoteEmptyDirsRecursiveSync(
          {required UuidValue sessionId,
          required String path,
          required bool includeHidden,
          dynamic hint}) =>
      emptyDirsDispatch.future;

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  String mainGetCommonSync({required String key, dynamic hint}) =>
      '${++_jobId}';
}

class _DeletingController extends FileController {
  final confirmations = <String>[];

  _DeletingController()
      : super(
          isLocal: false,
          getSessionID: () => _sessionId,
          rootState: WeakReference(_FakeFFI()),
          jobController: JobController(() => _sessionId, () => null),
          fileFetcher: FileFetcher(() => _sessionId),
          getOtherSideDirectoryData: () =>
              DirectoryData(FileDirectory(), DirectoryOptions()),
        );

  @override
  Future<bool?> showRemoveDialog(
      String title, String content, bool showCheckbox) async {
    confirmations.add(content);
    return true;
  }

  @override
  void sendRemoveFile(String path, int fileNum, int actId) {
    scheduleMicrotask(() {
      selectedItems.reconcile(
          selectedItems.items.where((entry) => entry.path != path).toList());
      unawaited(
          jobController.jobDone({'id': '$actId', 'file_num': '$fileNum'}));
    });
  }

  @override
  Future<bool> refresh() async => true;
}

void main() {
  for (final scenario in ['completion', 'reconnect', 'dispatch reconnect']) {
    testWidgets('SAF retry after $scenario', (tester) async {
      const path = '/@rustdesk/tree/grant/root';
      final response = jsonEncode({
        'path': path,
        'empty_dirs': [jsonDecode(_directoryJson('$path/empty'))],
      });
      final binding = _FileBinding();
      platformFFI.setBindingForTesting(binding);
      final fetcher = FileFetcher(() => _sessionId);
      final first = expectLater(
          fetcher.readEmptyDirs(path, false, false),
          scenario == 'completion'
              ? completion(isNotEmpty)
              : throwsA(isA<StateError>()));
      try {
        if (scenario != 'dispatch reconnect')
          binding.emptyDirsDispatch.complete();
        await tester.pump(const Duration(seconds: 3));
        if (scenario != 'completion') {
          fetcher.beginRemoteSession();
        } else {
          fetcher.tryCompleteEmptyDirsTask(response, 'false');
        }
        if (scenario == 'dispatch reconnect')
          binding.emptyDirsDispatch.complete();
        await tester.pump();
        final retry = expectLater(
            fetcher.readEmptyDirs(path, false, false).then((directories) =>
                directories.map((directory) => directory.path)),
            completion(['$path/empty']));
        await tester.pump();
        await tester.pump(const Duration(seconds: 28));
        fetcher.tryCompleteEmptyDirsTask(response, 'false');
        await tester.pump(const Duration(seconds: 2));
        await retry;
      } finally {
        await tester.pump(const Duration(seconds: 30));
      }
      await first;
    });
  }

  test('deletion confirms all five files despite refreshed selection',
      () async {
    platformFFI.setBindingForTesting(_FileBinding());
    final controller = _DeletingController();
    final files = ['a.txt', 'b.txt', 'c.txt', 'd.txt', 'e.txt']
        .map((name) => Entry()
          ..name = name
          ..path = '/selected/$name')
        .toList();
    controller.selectedItems.selectAll(files);

    await controller.removeAction(controller.selectedItems);

    expect(controller.confirmations, files.map((file) => file.path));
    expect(controller.selectedItems.items, isEmpty);
  });

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
