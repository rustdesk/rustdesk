import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../../common.dart';
import '../../consts.dart';
import '../../models/model.dart';
import '../widgets/tabbar_widget.dart';

class RemoteWindowResize {
  static final enabled = ValueNotifier<bool>(false);
  static const _channel =
      MethodChannel('org.rustdesk.rustdesk/remote-window-resize');

  final CanvasModel canvas;
  final bool Function() isSelected;
  StreamSubscription<DesktopTabState>? _tabs;

  RemoteWindowResize(
      this.canvas, this.isSelected, DesktopTabController? tabController) {
    if (!isWindows) return;
    canvas.addListener(_update);
    _tabs = tabController?.state.listen((_) => _update());
    _update();
  }

  void _update() {
    if (!isSelected()) return;
    final view = canvas.viewStyle;
    final width = canvas.getDisplayWidth();
    final height = canvas.getDisplayHeight();
    final adaptive =
        view.style == kRemoteViewStyleAdaptive && width > 0 && height > 0;
    enabled.value = adaptive;
    _send(adaptive ? width / height : 0.0);
  }

  void _send(double ratio) {
    unawaited(_channel.invokeMethod<void>('setContentGeometry', <double>[
      ratio,
      CanvasModel.leftToEdge + CanvasModel.rightToEdge,
      CanvasModel.topToEdge + CanvasModel.bottomToEdge,
    ]).catchError((Object error) {
      debugPrint('Remote window resize: $error');
    }));
  }

  void dispose() {
    if (!isWindows) return;
    canvas.removeListener(_update);
    _tabs?.cancel();
    if (isSelected()) _send(0.0);
  }
}
