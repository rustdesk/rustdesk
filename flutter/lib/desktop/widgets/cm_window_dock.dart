import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';
import 'package:window_size/window_size.dart' as window_size;

import '../../common.dart';
import '../../consts.dart';
import '../../models/platform_model.dart';
import '../../models/server_model.dart';
import '../../utils/platform_channel.dart';

final cmWindowDock = CmWindowDock();

class CmWindowDock extends ValueNotifier<bool> {
  CmWindowDock()
      : enabled = !kUseCompatibleUiMode &&
            bind.mainGetOptionSync(key: kOptionAllowAcceptWindowBanner) == 'Y',
        super(false);

  static const size = Size(36, 72);
  static const bottomInset = 64.0;
  static final compactSize =
      Size(kConnectionManagerWindowSizeClosedChat.width, 280);
  Future<void>? _transition;

  // Keep the mode fixed for the lifetime of this CM process.
  final bool enabled;

  Size get expandedSize {
    final contentSize = gFFI.chatModel.isShowCMSidePage
        ? kConnectionManagerWindowSizeOpenChat
        : kConnectionManagerWindowSizeClosedChat;
    return enabled ? Size(contentSize.width, expandedHeight) : contentSize;
  }

  double get expandedHeight {
    if (!enabled) return compactSize.height;
    final model = gFFI.serverModel;
    var extraRows = 0;
    for (final client in model.clients.where((client) => !client.disconnected)) {
      final elevate = model.showElevation &&
          client.type_() == ClientType.remote &&
          bind.cmCanElevate();
      var rows = 0;
      if (client.authorized) {
        rows = (client.inVoiceCall ? 1 : 0) +
            (client.incomingVoiceCall ? 1 : 0) +
            (client.fromSwitch ? 1 : 0) +
            (elevate ? 1 : 0);
      } else if (elevate &&
          model.approveMode != 'password' &&
          bind.mainGetBuildinOption(
                  key: kOptionHideElevateButtonInAcceptWindow) !=
              'Y') {
        rows = 1;
      }
      if (rows > extraRows) extraRows = rows;
    }
    // Button rows are 28 high, with a 4-pixel margin above and below.
    return compactSize.height + extraRows * 36;
  }

  Future<void> syncExpandedHeight() async {
    if (!enabled || value) return;
    final bounds = await windowManager.getBounds();
    if (value) return;
    final height = expandedHeight;
    if (bounds.height == height) return;
    await windowManager.setBounds(Rect.fromLTWH(
        bounds.left, bounds.bottom - height, bounds.width, height));
  }

  Future<void> syncMinimizable() async {
    final minimizable = !enabled;
    if (await windowManager.isMinimizable() != minimizable) {
      await windowManager.setMinimizable(minimizable);
      if (isMacOS) {
        if (enabled) {
          await windowManager.setAsFrameless();
        } else {
          await windowManager.setTitleBarStyle(TitleBarStyle.hidden,
              windowButtonVisibility: minimizable);
        }
      }
    }
  }

  Future<void> resizeExpanded(Size contentSize, {bool align = false}) async {
    final windowSize = Size(contentSize.width + size.width, expandedHeight);
    // On macOS, hiding just the title-bar buttons leaves the native title-bar
    // view over the transparent margin. Hide the whole view and its shadow.
    if (isMacOS || isWindows) {
      await windowManager.setAsFrameless();
    }
    if (align) {
      await windowManager.setSizeAlignment(windowSize, Alignment.bottomRight);
    } else {
      final bounds = await windowManager.getBounds();
      final bottom = bounds.bottom + (value ? bottomInset : 0);
      await windowManager.setBounds(Rect.fromLTWH(
        bounds.right - windowSize.width,
        bottom - windowSize.height,
        windowSize.width,
        windowSize.height,
      ));
    }
  }

  Future<void> dock({bool manual = false}) async {
    if (!enabled || value || _transition != null) return;
    final model = gFFI.serverModel;
    final connected = model.clients.where((client) => !client.disconnected);
    // Keep automatic folding from hiding requests that need attention, while
    // allowing an explicit click even when only disconnected tabs remain.
    if (model.hideCm ||
        (!manual &&
            (connected.isEmpty ||
                connected.any((client) =>
                    !client.authorized || client.incomingVoiceCall)))) return;
    model.cmHiddenTimer?.cancel();
    model.cmHiddenTimer = null;
    _transition = _dock();
    try {
      await _transition;
    } finally {
      _transition = null;
    }
  }

  Future<void> _dock() async {
    value = true;
    // Hide the full layout before the native window becomes smaller.
    await WidgetsBinding.instance.endOfFrame;
    final bounds = await windowManager.getBounds();
    final window = await window_size.getWindowInfo();
    final screen = window.screen;
    final right = screen == null
        ? bounds.right
        : screen.visibleFrame.right / (isWindows ? window.scaleFactor : 1);
    if (isWindows) await RdPlatformChannel.instance.setCmBannerWindow();
    if (isMacOS || isWindows) {
      await windowManager.setAsFrameless();
    }
    await windowManager.setBounds(Rect.fromLTWH(
      right - size.width,
      bounds.bottom - bottomInset - size.height,
      size.width,
      size.height,
    ));
    await windowManager.setAlwaysOnTop(true);
    await windowManager.show();
  }

  Future<void> expand() async {
    if (_transition != null) await _transition;
    if (!value) return;
    if (enabled) {
      await resizeExpanded(expandedSize);
    } else {
      await windowManager.setSizeAlignment(expandedSize, Alignment.topRight);
    }
    value = false;
  }
}

class CmWindowDockView extends StatelessWidget {
  const CmWindowDockView({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<bool>(
      valueListenable: cmWindowDock,
      child: child,
      builder: (context, docked, child) {
        if (!cmWindowDock.enabled && !docked) return child!;
        return CmDockLayout(
          docked: docked,
          contentSize: cmWindowDock.expandedSize,
          onToggle: () async {
            if (docked) {
              await cmWindowDock.expand();
              await windowManager.focus();
            } else {
              await cmWindowDock.dock(manual: true);
            }
          },
          child: child!,
        );
      },
    );
  }
}

class CmDockLayout extends StatelessWidget {
  const CmDockLayout({
    super.key,
    required this.docked,
    required this.contentSize,
    required this.onToggle,
    required this.child,
  });

  final bool docked;
  final Size contentSize;
  final VoidCallback onToggle;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
        builder: (context, constraints) => Stack(
              children: [
                Positioned(
                  left: docked ? 0 : CmWindowDock.size.width,
                  top: 0,
                  right: 0,
                  bottom: 0,
                  // Keep the connection controls mounted, at their normal size.
                  child: ExcludeFocus(
                    excluding: docked,
                    child: Offstage(
                      offstage: docked,
                      child: OverflowBox(
                        minWidth: docked ? contentSize.width : null,
                        maxWidth: docked ? contentSize.width : null,
                        minHeight: docked ? contentSize.height : null,
                        maxHeight: docked ? contentSize.height : null,
                        child: child,
                      ),
                    ),
                  ),
                ),
                Positioned(
                  left: 0,
                  bottom: constraints.maxHeight > CmWindowDock.size.height
                      ? CmWindowDock.bottomInset
                      : 0,
                  child: CmDockTab(docked: docked, onTap: onToggle),
                ),
              ],
            ));
  }
}

class CmDockTab extends StatelessWidget {
  const CmDockTab({super.key, required this.docked, required this.onTap});

  final bool docked;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return ClipPath(
      clipper: const _CmDockTabClipper(),
      child: Material(
        color: Theme.of(context).scaffoldBackgroundColor,
        child: Semantics(
          button: true,
          label: translate(
              docked ? 'Restore' : 'Dock connection manager to the right'),
          child: InkWell(
            onTap: onTap,
            child: SizedBox(
              // Join the tab to the panel across its one-pixel border.
              width: CmWindowDock.size.width + (docked ? 0 : 1),
              height: CmWindowDock.size.height,
              child: CustomPaint(
                foregroundPainter: docked
                    ? null
                    : _CmDockTabBorder(MyTheme.color(context).border!),
                child: Align(
                  alignment: const Alignment(0, -0.45),
                  child: docked
                      ? loadIcon(24)
                      : const Icon(Icons.chevron_right,
                          size: 22, textDirection: TextDirection.ltr),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _CmDockTabBorder extends CustomPainter {
  const _CmDockTabBorder(this.color);

  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    // The outer ClipPath keeps one pixel of this stroke inside the tab;
    // leave the overlap with the panel open so no divider crosses the join.
    canvas.save();
    canvas.clipRect(Rect.fromLTWH(0, 0, size.width - 1, size.height));
    canvas.drawPath(
        const _CmDockTabClipper().getClip(size),
        Paint()
          ..color = color
          ..style = PaintingStyle.stroke
          ..strokeWidth = 2);
    canvas.restore();
  }

  @override
  bool shouldRepaint(_CmDockTabBorder oldDelegate) => color != oldDelegate.color;
}

class _CmDockTabClipper extends CustomClipper<Path> {
  const _CmDockTabClipper();

  @override
  Path getClip(Size size) => Path()
    ..moveTo(size.width, 0)
    ..lineTo(8, 0)
    ..quadraticBezierTo(0, 0, 0, 8)
    ..lineTo(0, size.height - 30)
    ..quadraticBezierTo(0, size.height - 24, 6, size.height - 20)
    ..lineTo(size.width, size.height)
    ..close();

  @override
  bool shouldReclip(_CmDockTabClipper oldClipper) => false;
}
