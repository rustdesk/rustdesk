import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';

import '../../desktop/widgets/tabbar_widget.dart';
import '../../mobile/pages/chat_page.dart';
import '../../models/chat_model.dart';

class DraggableChatWindow extends StatelessWidget {
  const DraggableChatWindow(
      {Key? key,
      this.position = Offset.zero,
      required this.width,
      required this.height,
      required this.chatModel})
      : super(key: key);

  final Offset position;
  final double width;
  final double height;
  final ChatModel chatModel;

  @override
  Widget build(BuildContext context) {
    return Draggable(
        checkKeyboard: true,
        position: position,
        width: width,
        height: height,
        builder: (context, onPanUpdate) {
          return isIOS
              ? ChatPage(chatModel: chatModel)
              : Scaffold(
                  resizeToAvoidBottomInset: false,
                  appBar: CustomAppBar(
                    onPanUpdate: onPanUpdate,
                    appBar: isDesktop
                        ? _buildDesktopAppBar()
                        : _buildMobileAppBar(context),
                  ),
                  body: ChatPage(chatModel: chatModel),
                );
        });
  }

  Widget _buildMobileAppBar(BuildContext context) {
    return Container(
      color: Theme.of(context).colorScheme.primary,
      height: 50,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 15),
              child: Text(
                translate("Chat"),
                style: const TextStyle(
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
                    chatModel.hideChatWindowOverlay();
                  },
                  icon: const Icon(Icons.keyboard_arrow_down)),
              IconButton(
                  onPressed: () {
                    chatModel.hideChatWindowOverlay();
                    chatModel.hideChatIconOverlay();
                  },
                  icon: const Icon(Icons.close))
            ],
          )
        ],
      ),
    );
  }

  Widget _buildDesktopAppBar() {
    return Container(
      color: MyTheme.accent50,
      height: 35,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 15),
              child: Text(
                translate("Chat"),
                style: const TextStyle(
                    color: Colors.white,
                    fontFamily: 'WorkSans',
                    fontWeight: FontWeight.bold),
              )),
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              ActionIcon(
                message: 'Close',
                icon: IconFont.close,
                onTap: chatModel.hideChatWindowOverlay,
                isClose: true,
              )
            ],
          )
        ],
      ),
    );
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
  Size get preferredSize => const Size.fromHeight(kToolbarHeight);
}

/// floating buttons of back/home/recent actions for android
class DraggableMobileActions extends StatelessWidget {
  DraggableMobileActions(
      {this.position = Offset.zero,
      this.onBackPressed,
      this.onRecentPressed,
      this.onHomePressed,
      this.onHidePressed,
      required this.width,
      required this.height});

  final Offset position;
  final double width;
  final double height;
  final VoidCallback? onBackPressed;
  final VoidCallback? onHomePressed;
  final VoidCallback? onRecentPressed;
  final VoidCallback? onHidePressed;

  @override
  Widget build(BuildContext context) {
    return Draggable(
        position: position,
        width: width,
        height: height,
        builder: (_, onPanUpdate) {
          return GestureDetector(
              onPanUpdate: onPanUpdate,
              child: Card(
                  color: Colors.transparent,
                  shadowColor: Colors.transparent,
                  child: Container(
                    decoration: BoxDecoration(
                        color: MyTheme.accent.withOpacity(0.4),
                        borderRadius: BorderRadius.all(Radius.circular(15))),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceAround,
                      children: [
                        IconButton(
                            color: Colors.white,
                            onPressed: onBackPressed,
                            splashRadius: 20,
                            icon: const Icon(Icons.arrow_back)),
                        IconButton(
                            color: Colors.white,
                            onPressed: onHomePressed,
                            splashRadius: 20,
                            icon: const Icon(Icons.home)),
                        IconButton(
                            color: Colors.white,
                            onPressed: onRecentPressed,
                            splashRadius: 20,
                            icon: const Icon(Icons.more_horiz)),
                        const VerticalDivider(
                          width: 0,
                          thickness: 2,
                          indent: 10,
                          endIndent: 10,
                        ),
                        IconButton(
                            color: Colors.white,
                            onPressed: onHidePressed,
                            splashRadius: 20,
                            icon: const Icon(Icons.keyboard_arrow_down)),
                      ],
                    ),
                  )));
        });
  }
}

class Draggable extends StatefulWidget {
  const Draggable(
      {Key? key,
      this.checkKeyboard = false,
      this.checkScreenSize = false,
      this.position = Offset.zero,
      required this.width,
      required this.height,
      required this.builder})
      : super(key: key);

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
