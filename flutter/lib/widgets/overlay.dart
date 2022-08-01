import 'package:draggable_float_widget/draggable_float_widget.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';

import '../models/model.dart';
import '../pages/chat_page.dart';

OverlayEntry? chatIconOverlayEntry;
OverlayEntry? chatWindowOverlayEntry;

OverlayEntry? mobileActionsOverlayEntry;

class DraggableChatWindow extends StatelessWidget {
  DraggableChatWindow(
      {this.position = Offset.zero, required this.width, required this.height});

  final Offset position;
  final double width;
  final double height;

  @override
  Widget build(BuildContext context) {
    return Draggable(
        checkKeyboard: true,
        position: position,
        width: width,
        height: height,
        builder: (_, onPanUpdate) {
          return isIOS
              ? ChatPage()
              : Scaffold(
                  resizeToAvoidBottomInset: false,
                  appBar: CustomAppBar(
                    onPanUpdate: onPanUpdate,
                    appBar: Container(
                      color: MyTheme.accent50,
                      height: 50,
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          Padding(
                              padding: EdgeInsets.symmetric(horizontal: 15),
                              child: Text(
                                translate("Chat"),
                                style: TextStyle(
                                    color: Colors.white,
                                    fontFamily: 'WorkSans',
                                    fontWeight: FontWeight.bold,
                                    fontSize: 20),
                              )),
                          Row(
                            crossAxisAlignment: CrossAxisAlignment.center,
                            children: [
                              IconButton(
                                  onPressed: () {
                                    hideChatWindowOverlay();
                                  },
                                  icon: Icon(Icons.keyboard_arrow_down)),
                              IconButton(
                                  onPressed: () {
                                    hideChatWindowOverlay();
                                    hideChatIconOverlay();
                                  },
                                  icon: Icon(Icons.close))
                            ],
                          )
                        ],
                      ),
                    ),
                  ),
                  body: ChatPage(),
                );
        });
  }
}

class CustomAppBar extends StatelessWidget implements PreferredSizeWidget {
  final GestureDragUpdateCallback onPanUpdate;
  final Widget appBar;

  const CustomAppBar(
      {Key? key, required this.onPanUpdate, required this.appBar})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return GestureDetector(onPanUpdate: onPanUpdate, child: appBar);
  }

  @override
  Size get preferredSize => new Size.fromHeight(kToolbarHeight);
}

showChatIconOverlay({Offset offset = const Offset(200, 50)}) {
  if (chatIconOverlayEntry != null) {
    chatIconOverlayEntry!.remove();
  }
  if (globalKey.currentState == null || globalKey.currentState!.overlay == null)
    return;
  final bar = navigationBarKey.currentWidget;
  if (bar != null) {
    if ((bar as BottomNavigationBar).currentIndex == 1) {
      return;
    }
  }
  final globalOverlayState = globalKey.currentState!.overlay!;

  final overlay = OverlayEntry(builder: (context) {
    return DraggableFloatWidget(
        config: DraggableFloatWidgetBaseConfig(
          initPositionYInTop: false,
          initPositionYMarginBorder: 100,
          borderTopContainTopBar: true,
        ),
        child: FloatingActionButton(
            onPressed: () {
              if (chatWindowOverlayEntry == null) {
                showChatWindowOverlay();
              } else {
                hideChatWindowOverlay();
              }
            },
            child: Icon(Icons.message)));
  });
  globalOverlayState.insert(overlay);
  chatIconOverlayEntry = overlay;
}

hideChatIconOverlay() {
  if (chatIconOverlayEntry != null) {
    chatIconOverlayEntry!.remove();
    chatIconOverlayEntry = null;
  }
}

showChatWindowOverlay() {
  if (chatWindowOverlayEntry != null) return;
  if (globalKey.currentState == null || globalKey.currentState!.overlay == null)
    return;
  final globalOverlayState = globalKey.currentState!.overlay!;

  final overlay = OverlayEntry(builder: (context) {
    return DraggableChatWindow(
        position: Offset(20, 80), width: 250, height: 350);
  });
  globalOverlayState.insert(overlay);
  chatWindowOverlayEntry = overlay;
}

hideChatWindowOverlay() {
  if (chatWindowOverlayEntry != null) {
    chatWindowOverlayEntry!.remove();
    chatWindowOverlayEntry = null;
    return;
  }
}

toggleChatOverlay() {
  if (chatIconOverlayEntry == null || chatWindowOverlayEntry == null) {
    FFI.invokeMethod("enable_soft_keyboard", true);
    showChatIconOverlay();
    showChatWindowOverlay();
  } else {
    hideChatIconOverlay();
    hideChatWindowOverlay();
  }
}

/// floating buttons of back/home/recent actions for android
class DraggableMobileActions extends StatelessWidget {
  DraggableMobileActions(
      {this.position = Offset.zero,
      this.onBackPressed,
      this.onRecentPressed,
      this.onHomePressed,
      required this.width,
      required this.height});

  final Offset position;
  final double width;
  final double height;
  final VoidCallback? onBackPressed;
  final VoidCallback? onHomePressed;
  final VoidCallback? onRecentPressed;

  @override
  Widget build(BuildContext context) {
    return Draggable(
        position: position,
        width: width,
        height: height,
        builder: (_, onPanUpdate) {
          return GestureDetector(
              onPanUpdate: onPanUpdate,
              child: Container(
                decoration: BoxDecoration(
                    color: MyTheme.accent.withOpacity(0.4),
                    borderRadius: BorderRadius.all(Radius.circular(15))),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceAround,
                  children: [
                    IconButton(
                        color: MyTheme.white,
                        onPressed: onBackPressed,
                        icon: Icon(Icons.arrow_back)),
                    IconButton(
                        color: MyTheme.white,
                        onPressed: onHomePressed,
                        icon: Icon(Icons.home)),
                    IconButton(
                        color: MyTheme.white,
                        onPressed: onRecentPressed,
                        icon: Icon(Icons.more_horiz)),
                    VerticalDivider(
                      width: 0,
                      thickness: 2,
                      indent: 10,
                      endIndent: 10,
                    ),
                    IconButton(
                        color: MyTheme.white,
                        onPressed: hideMobileActionsOverlay,
                        icon: Icon(Icons.keyboard_arrow_down)),
                  ],
                ),
              ));
        });
  }
}

resetMobileActionsOverlay() {
  if (mobileActionsOverlayEntry == null) return;
  hideMobileActionsOverlay();
  showMobileActionsOverlay();
}

showMobileActionsOverlay() {
  if (mobileActionsOverlayEntry != null) return;
  if (globalKey.currentContext == null ||
      globalKey.currentState == null ||
      globalKey.currentState!.overlay == null) return;
  final globalOverlayState = globalKey.currentState!.overlay!;

  // compute overlay position
  final screenW = MediaQuery.of(globalKey.currentContext!).size.width;
  final screenH = MediaQuery.of(globalKey.currentContext!).size.height;
  final double overlayW = 200;
  final double overlayH = 45;
  final left = (screenW - overlayW) / 2;
  final top = screenH - overlayH - 80;

  final overlay = OverlayEntry(builder: (context) {
    return DraggableMobileActions(
      position: Offset(left, top),
      width: overlayW,
      height: overlayH,
      onBackPressed: () => FFI.tap(MouseButtons.right),
      onHomePressed: () => FFI.tap(MouseButtons.wheel),
      onRecentPressed: () async {
        FFI.sendMouse('down', MouseButtons.wheel);
        await Future.delayed(Duration(milliseconds: 500));
        FFI.sendMouse('up', MouseButtons.wheel);
      },
    );
  });
  globalOverlayState.insert(overlay);
  mobileActionsOverlayEntry = overlay;
}

hideMobileActionsOverlay() {
  if (mobileActionsOverlayEntry != null) {
    mobileActionsOverlayEntry!.remove();
    mobileActionsOverlayEntry = null;
    return;
  }
}

class Draggable extends StatefulWidget {
  Draggable(
      {this.checkKeyboard = false,
      this.checkScreenSize = false,
      this.position = Offset.zero,
      required this.width,
      required this.height,
      required this.builder});

  final bool checkKeyboard;
  final bool checkScreenSize;
  final Offset position;
  final double width;
  final double height;
  final Widget Function(BuildContext, GestureDragUpdateCallback) builder;

  @override
  State<StatefulWidget> createState() => _DraggableState();
}

class _DraggableState extends State<Draggable> {
  late Offset _position;
  bool _keyboardVisible = false;
  double _saveHeight = 0;
  double _lastBottomHeight = 0;

  @override
  void initState() {
    super.initState();
    _position = widget.position;
  }

  void onPanUpdate(DragUpdateDetails d) {
    final offset = d.delta;
    final size = MediaQuery.of(context).size;
    double x = 0;
    double y = 0;

    if (_position.dx + offset.dx + widget.width > size.width) {
      x = size.width - widget.width;
    } else if (_position.dx + offset.dx < 0) {
      x = 0;
    } else {
      x = _position.dx + offset.dx;
    }

    if (_position.dy + offset.dy + widget.height > size.height) {
      y = size.height - widget.height;
    } else if (_position.dy + offset.dy < 0) {
      y = 0;
    } else {
      y = _position.dy + offset.dy;
    }
    setState(() {
      _position = Offset(x, y);
    });
  }

  checkScreenSize() {}

  checkKeyboard() {
    final bottomHeight = MediaQuery.of(context).viewInsets.bottom;
    final currentVisible = bottomHeight != 0;

    debugPrint(bottomHeight.toString() + currentVisible.toString());
    // save
    if (!_keyboardVisible && currentVisible) {
      _saveHeight = _position.dy;
    }

    // reset
    if (_lastBottomHeight > 0 && bottomHeight == 0) {
      setState(() {
        _position = Offset(_position.dx, _saveHeight);
      });
    }

    // onKeyboardVisible
    if (_keyboardVisible && currentVisible) {
      final sumHeight = bottomHeight + widget.height;
      final contextHeight = MediaQuery.of(context).size.height;
      if (sumHeight + _position.dy > contextHeight) {
        final y = contextHeight - sumHeight;
        setState(() {
          _position = Offset(_position.dx, y);
        });
      }
    }

    _keyboardVisible = currentVisible;
    _lastBottomHeight = bottomHeight;
  }

  @override
  Widget build(BuildContext context) {
    if (widget.checkKeyboard) {
      checkKeyboard();
    }
    if (widget.checkKeyboard) {
      checkScreenSize();
    }
    return Positioned(
        top: _position.dy,
        left: _position.dx,
        width: widget.width,
        height: widget.height,
        child: widget.builder(context, onPanUpdate));
  }
}
