import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_test/flutter_test.dart' hide isLinux;
import 'package:window_manager/window_manager.dart';

void main() {
  test('maximized Linux window keeps its layout after minimize and restore',
      () {
    final listener = DesktopTab(
      controller: DesktopTabController(tabType: DesktopTabType.remoteScreen),
    ).createState() as WindowListener;
    addTearDown(() {
      stateGlobal.setMinimized(false);
      stateGlobal.setMaximized(false);
    });
    stateGlobal.setMaximized(true);

    listener.onWindowMinimize();
    listener.onWindowRestore();

    expect(kDragToResizeAreaPadding, EdgeInsets.zero);
    expect(stateGlobal.windowBorderWidth.value, 0);
  }, skip: !isLinux);
}
