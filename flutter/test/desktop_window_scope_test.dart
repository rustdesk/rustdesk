import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/window_scope.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('a dialog closes only its owning session for the same peer', () {
    var remoteClosed = false;
    var cameraClosed = false;
    final remoteDialogs = OverlayDialogManager()
      ..onCloseSession = () => remoteClosed = true;
    final cameraDialogs = OverlayDialogManager()
      ..onCloseSession = () => cameraClosed = true;

    cameraDialogs.closeSession(id: 'same-peer');
    expect(cameraClosed, isTrue);
    expect(remoteClosed, isFalse);

    remoteDialogs.closeSession(id: 'same-peer');
    expect(remoteClosed, isTrue);
  });

  testWidgets('desktop session chrome state belongs to its own window',
      (tester) async {
    final remote = StateGlobal.forDesktopWindow(7);
    final files = StateGlobal.forDesktopWindow(8);
    final scopes = <int, DesktopWindowScope>{};

    Widget window(StateGlobal state, WindowType type) => DesktopWindowScope(
          windowId: state.windowId,
          windowType: type,
          windowState: state,
          child: Builder(builder: (context) {
            final scope = DesktopWindowScope.read(context)!;
            scopes[scope.windowId] = scope;
            return const SizedBox();
          }),
        );

    await tester.pumpWidget(Directionality(
      textDirection: TextDirection.ltr,
      child: Row(children: [
        window(remote, WindowType.RemoteDesktop),
        window(files, WindowType.FileTransfer),
      ]),
    ));

    expect(scopes[7]!.windowType, WindowType.RemoteDesktop);
    expect(scopes[8]!.windowType, WindowType.FileTransfer);
    scopes[7]!.windowState.isFocused.value = true;
    scopes[7]!.windowState.setMaximized(true);
    scopes[8]!.windowState.setMinimized(true);

    expect(remote.isFocused.value, isTrue);
    expect(remote.isMaximized.value, isTrue);
    expect(remote.isMinimized, isFalse);
    expect(files.isFocused.value, isFalse);
    expect(files.isMaximized.value, isFalse);
    expect(files.isMinimized, isTrue);
  });
}
