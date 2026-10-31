import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/install_page.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:path/path.dart' as path;

class _Binding extends Fake implements RustdeskImpl {
  _Binding(this.directory);

  final String directory;
  final installedPaths = <String>[];

  @override
  String installInstallPath({dynamic hint}) => directory;
  @override
  String installInstallOptions({dynamic hint}) => '{}';
  @override
  bool installShowRunWithoutInstall({dynamic hint}) => true;
  @override
  bool isIncomingOnly({dynamic hint}) => false;
  @override
  bool isOutgoingOnly({dynamic hint}) => false;
  @override
  String mainGetLocalOption({required String key, dynamic hint}) => '';
  @override
  String mainGetBuildinOption({required String key, dynamic hint}) => '';
  @override
  String mainGetAppNameSync({dynamic hint}) => 'RustDesk';
  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;
  @override
  Future<void> installInstallMe(
      {required String options, required String path, dynamic hint}) async {
    installedPaths.add(path);
  }
}

void main() {
  const inlineWarning = 'The selected folder is not empty. '
      'Installation may delete existing data. '
      'Consider choosing a different folder.';
  final testBinding = TestWidgetsFlutterBinding.ensureInitialized();
  late Directory root;
  late Directory existing;
  late _Binding binding;

  setUp(() {
    root = Directory.systemTemp.createTempSync('rustdesk-install-confirm-');
    existing = Directory(path.join(root.path, 'Existing'))..createSync();
    File(path.join(existing.path, 'notes.txt')).writeAsStringSync('keep');
    binding = _Binding(existing.path);
    platformFFI.setBindingForTesting(binding);
    testBinding.defaultBinaryMessenger.setMockMethodCallHandler(
        const MethodChannel('window_manager'), (call) async => false);
  });

  tearDown(() {
    root.deleteSync(recursive: true);
    testBinding.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('window_manager'), null);
  });

  Future<void> openInstaller(WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      theme: MyTheme.lightTheme,
      home: const InstallPage(),
    ));
    await tester.pump(const Duration(seconds: 1));
    await tester.pumpAndSettle();
  }

  Future<void> pressInstall(WidgetTester tester) async {
    await tester.ensureVisible(find.text('Accept and Install'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Accept and Install'));
    while (find.byType(AlertDialog).evaluate().isEmpty &&
        binding.installedPaths.isEmpty) {
      await Future<void>.delayed(const Duration(milliseconds: 10));
      await tester.pump();
    }
    if (binding.installedPaths.isEmpty) await tester.pumpAndSettle();
  }

  Future<void> settleDirectoryChecks(WidgetTester tester) async {
    await tester.pump();
    for (final builder in tester
        .widgetList<FutureBuilder<bool>>(find.byType(FutureBuilder<bool>))) {
      await builder.future;
    }
    await tester.pumpAndSettle();
  }

  Finder button(String label) => find.ancestor(
      of: find.text(label),
      matching:
          find.byWidgetPredicate((widget) => widget is ButtonStyleButton));

  Finder warningFor(String directory) => find.descendant(
      of: find.byType(AlertDialog), matching: find.textContaining(directory));

  testWidgets(
      'selected nonempty directory shows an inline warning below the path',
      (tester) => tester.runAsync(() async {
            await openInstaller(tester);
            await settleDirectoryChecks(tester);
            expect(find.text(inlineWarning), findsOneWidget);
            const minimumWarningGap = 8.0;
            const maximumWarningGap = 16.0;
            final warningPosition = tester.getTopLeft(find.text(inlineWarning));
            final pathBottom = tester.getBottomLeft(find.byType(TextField));
            expect(warningPosition.dy - pathBottom.dy,
                inInclusiveRange(minimumWarningGap, maximumWarningGap));
            expect(warningPosition.dx, pathBottom.dx);
            expect(find.byType(AlertDialog), findsNothing);

            final controller =
                tester.widget<TextField>(find.byType(TextField)).controller!;
            final empty = Directory(path.join(root.path, 'Empty'))
              ..createSync();
            for (final directory in [
              empty.path,
              path.join(root.path, 'Missing')
            ]) {
              controller.text = directory;
              await settleDirectoryChecks(tester);
              expect(find.text(inlineWarning), findsNothing);
            }
            controller.text = existing.path;
            await settleDirectoryChecks(tester);
            expect(find.text(inlineWarning), findsOneWidget);

            await pressInstall(tester);
            expect(warningFor(existing.path), findsOneWidget);
            expect(binding.installedPaths, isEmpty);
            await tester.tap(find.descendant(
                of: find.byType(AlertDialog), matching: find.text('Cancel')));
            await tester.pumpAndSettle();
          }));

  testWidgets(
      'canceling installation preserves files and permits another path',
      (tester) => tester.runAsync(() async {
            await openInstaller(tester);
            await pressInstall(tester);

            expect(binding.installedPaths, isEmpty);
            expect(warningFor(existing.path), findsOneWidget);
            expect(
                find.descendant(
                    of: find.byType(AlertDialog),
                    matching: find.text('$inlineWarning\n${existing.path}')),
                findsOneWidget);
            await tester.tap(find.descendant(
                of: find.byType(AlertDialog), matching: find.text('Cancel')));
            await tester.pumpAndSettle();

            final changePath =
                tester.widget<ButtonStyleButton>(button('Change Path'));
            expect(changePath.onPressed, isNotNull);

            await pressInstall(tester);
            await tester.sendKeyEvent(LogicalKeyboardKey.escape);
            await tester.pumpAndSettle();
            expect(find.byType(AlertDialog), findsNothing);
            expect(binding.installedPaths, isEmpty);
          }));

  testWidgets(
      'relocation warns about the old folder and requires confirmation',
      (tester) => tester.runAsync(() async {
            await openInstaller(tester);
            final destination = Directory(path.join(root.path, 'New'));
            tester.widget<TextField>(find.byType(TextField)).controller!.text =
                destination.path;
            await pressInstall(tester);
            expect(find.textContaining(existing.path), findsOneWidget);
            await tester.tap(find.descendant(
                of: find.byType(AlertDialog), matching: find.text('Cancel')));
            await tester.pumpAndSettle();

            destination.createSync();
            File(path.join(destination.path, 'notes.txt'))
                .writeAsStringSync('keep');
            await pressInstall(tester);
            expect(warningFor(destination.path), findsOneWidget);
            expect(find.textContaining('$inlineWarning\n${destination.path}'),
                findsOneWidget);
            expect(
                find.textContaining(
                    'Installation will delete this folder and all its contents:\n${existing.path}'),
                findsOneWidget);
            expect(binding.installedPaths, isEmpty);
            await tester.tap(find.text('Continue'));
            await tester.pump();
            expect(binding.installedPaths, [destination.path]);
          }));

  testWidgets(
      'directory inspection errors stop installation visibly',
      (tester) => tester.runAsync(() async {
            await openInstaller(tester);
            await IOOverrides.runZoned(
              () => pressInstall(tester),
              createDirectory: (value) =>
                  throw FileSystemException('Permission denied', value),
            );
            expect(find.text('Error'), findsOneWidget);
            expect(find.textContaining('Permission denied'), findsOneWidget);
            expect(binding.installedPaths, isEmpty);
            await tester.tap(find.text('OK'));
            await tester.pumpAndSettle();
          }));
}
