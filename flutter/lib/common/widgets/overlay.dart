import 'package:auto_size_text/auto_size_text.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';

import '../../consts.dart';
import '../../desktop/widgets/tabbar_widget.dart';
import '../../models/chat_model.dart';
import '../../models/model.dart';
import 'chat_page.dart';

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
                        ? _buildDesktopAppBar(context)
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

  Widget _buildDesktopAppBar(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
          border: Border(
              bottom: BorderSide(
                  color: Theme.of(context).hintColor.withOpacity(0.4)))),
      height: 38,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Padding(
              padding: const EdgeInsets.symmetric(horizontal: 15, vertical: 8),
              child: Obx(() => Opacity(
                  opacity: chatModel.isWindowFocus.value ? 1.0 : 0.4,
                  child: Row(children: [
                    Icon(Icons.chat_bubble_outline,
                        size: 20, color: Theme.of(context).colorScheme.primary),
                    SizedBox(width: 6),
                    Text(translate("Chat"))
                  ])))),
          Padding(
              padding: EdgeInsets.all(2),
              child: ActionIcon(
                message: 'Close',
                icon: IconFont.close,
                onTap: chatModel.hideChatWindowOverlay,
                isClose: true,
                boxSize: 32,
              ))
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
                            splashRadius: kDesktopIconButtonSplashRadius,
                            icon: const Icon(Icons.arrow_back)),
                        IconButton(
                            color: Colors.white,
                            onPressed: onHomePressed,
                            splashRadius: kDesktopIconButtonSplashRadius,
                            icon: const Icon(Icons.home)),
                        IconButton(
                            color: Colors.white,
                            onPressed: onRecentPressed,
                            splashRadius: kDesktopIconButtonSplashRadius,
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
                            splashRadius: kDesktopIconButtonSplashRadius,
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
    if (widget.checkScreenSize) {
      checkScreenSize();
    }
    return Stack(children: [
      Positioned(
          top: _position.dy,
          left: _position.dx,
          width: widget.width,
          height: widget.height,
          child: widget.builder(context, onPanUpdate))
    ]);
  }
}

class QualityMonitor extends StatelessWidget {
  final QualityMonitorModel qualityMonitorModel;
  QualityMonitor(this.qualityMonitorModel);

  Widget _row(String info, String? value, {Color? rightColor}) {
    return Row(
      children: [
        Expanded(
            flex: 8,
            child: AutoSizeText(info,
                style: TextStyle(color: Color.fromARGB(255, 210, 210, 210)),
                textAlign: TextAlign.right,
                maxLines: 1)),
        Spacer(flex: 1),
        Expanded(
            flex: 8,
            child: AutoSizeText(value ?? '',
                style: TextStyle(color: rightColor ?? Colors.white),
                maxLines: 1)),
      ],
    );
  }

  @override
  Widget build(BuildContext context) => ChangeNotifierProvider.value(
      value: qualityMonitorModel,
      child: Consumer<QualityMonitorModel>(
          builder: (context, qualityMonitorModel, child) => qualityMonitorModel
                  .show
              ? Container(
                  constraints: BoxConstraints(maxWidth: 200),
                  padding: const EdgeInsets.all(8),
                  color: MyTheme.canvasColor.withAlpha(150),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      _row("Speed", qualityMonitorModel.data.speed ?? '-'),
                      _row("FPS", qualityMonitorModel.data.fps ?? '-'),
                      _row(
                          "Delay", "${qualityMonitorModel.data.delay ?? '-'}ms",
                          rightColor: Colors.green),
                      _row("Target Bitrate",
                          "${qualityMonitorModel.data.targetBitrate ?? '-'}kb"),
                      _row(
                          "Codec", qualityMonitorModel.data.codecFormat ?? '-'),
                    ],
                  ),
                )
              : const SizedBox.shrink()));
}

class BlockableOverlayState extends OverlayKeyState {
  final _middleBlocked = false.obs;

  VoidCallback? onMiddleBlockedClick; // to-do use listener

  RxBool get middleBlocked => _middleBlocked;

  void addMiddleBlockedListener(void Function(bool) cb) {
    _middleBlocked.listen(cb);
  }

  void setMiddleBlocked(bool blocked) {
    if (blocked != _middleBlocked.value) {
      _middleBlocked.value = blocked;
    }
  }

  void applyFfi(FFI ffi) {
    ffi.dialogManager.setOverlayState(this);
    ffi.chatModel.setOverlayState(this);
    // make remote page penetrable automatically, effective for chat over remote
    onMiddleBlockedClick = () {
      setMiddleBlocked(false);
    };
  }
}

class BlockableOverlay extends StatelessWidget {
  final Widget underlying;
  final List<OverlayEntry>? upperLayer;

  final BlockableOverlayState state;

  BlockableOverlay(
      {required this.underlying, required this.state, this.upperLayer});

  @override
  Widget build(BuildContext context) {
    final initialEntries = [
      OverlayEntry(builder: (_) => underlying),

      /// middle layer
      OverlayEntry(
          builder: (context) => Obx(() => Listener(
              onPointerDown: (_) {
                state.onMiddleBlockedClick?.call();
              },
              child: Container(
                  color:
                      state.middleBlocked.value ? Colors.transparent : null)))),
    ];

    if (upperLayer != null) {
      initialEntries.addAll(upperLayer!);
    }

    /// set key
    return Overlay(key: state.key, initialEntries: initialEntries);
  }
}
