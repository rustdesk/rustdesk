import 'package:dash_chat/dash_chat.dart';
import 'package:draggable_float_widget/draggable_float_widget.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:provider/provider.dart';
import '../models/model.dart';
import 'home_page.dart';

OverlayEntry? iconOverlayEntry;
OverlayEntry? windowOverlayEntry;

ChatPage chatPage = ChatPage();

class ChatPage extends StatelessWidget implements PageShape {
  @override
  final title = translate("Chat");

  @override
  final icon = Icon(Icons.chat);

  @override
  final appBarActions = [
    PopupMenuButton<int>(
        icon: Icon(Icons.group),
        itemBuilder: (context) {
          final chatModel = FFI.chatModel;
          final serverModel = FFI.serverModel;
          return chatModel.messages.entries.map((entry) {
            final id = entry.key;
            final user = serverModel.clients[id]?.chatUser ?? chatModel.me;
            return PopupMenuItem<int>(
              child: Text("${user.name}   ${user.uid}"),
              value: id,
            );
          }).toList();
        },
        onSelected: (id) {
          FFI.chatModel.changeCurrentID(id);
        })
  ];

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
        value: FFI.chatModel,
        child: Container(
            color: MyTheme.grayBg,
            child: Consumer<ChatModel>(builder: (context, chatModel, child) {
              final currentUser = chatModel.currentUser;
              return Stack(
                children: [
                  DashChat(
                    inputContainerStyle: BoxDecoration(color: Colors.white70),
                    sendOnEnter: false,
                    // if true,reload keyboard everytime,need fix
                    onSend: (chatMsg) {
                      chatModel.send(chatMsg);
                    },
                    user: chatModel.me,
                    messages: chatModel.messages[chatModel.currentID] ?? [],
                    // default scrollToBottom has bug https://github.com/fayeed/dash_chat/issues/53
                    scrollToBottom: false,
                    scrollController: chatModel.scroller,
                  ),
                  chatModel.currentID == ChatModel.clientModeID
                      ? SizedBox.shrink()
                      : Padding(
                          padding: EdgeInsets.all(12),
                          child: Row(
                            children: [
                              Icon(Icons.account_circle,
                                  color: MyTheme.accent80),
                              SizedBox(width: 5),
                              Text(
                                "${currentUser.name ?? ""}   ${currentUser.uid ?? ""}",
                                style: TextStyle(color: MyTheme.accent50),
                              ),
                            ],
                          )),
                ],
              );
            })));
  }
}

showChatIconOverlay({Offset offset = const Offset(200, 50)}) {
  if (iconOverlayEntry != null) {
    iconOverlayEntry!.remove();
  }
  if (globalKey.currentState == null || globalKey.currentState!.overlay == null)
    return;
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
              if (windowOverlayEntry == null) {
                showChatWindowOverlay();
              } else {
                hideChatWindowOverlay();
              }
            },
            child: Icon(Icons.message)));
  });
  globalOverlayState.insert(overlay);
  iconOverlayEntry = overlay;
  debugPrint("created");
}

hideChatIconOverlay() {
  if (iconOverlayEntry != null) {
    iconOverlayEntry!.remove();
    iconOverlayEntry = null;
  }
}

final FocusNode _focusNode = FocusNode();

showChatWindowOverlay() {
  if (windowOverlayEntry != null) return;
  if (globalKey.currentState == null || globalKey.currentState!.overlay == null)
    return;
  final globalOverlayState = globalKey.currentState!.overlay!;

  final overlay = OverlayEntry(builder: (context) {
    return ChatWindowOverlay();
  });
  _focusNode.requestFocus();
  globalOverlayState.insert(overlay);
  windowOverlayEntry = overlay;
  debugPrint("chatEntry created");
}

hideChatWindowOverlay() {
  if (windowOverlayEntry != null) {
    windowOverlayEntry!.remove();
    windowOverlayEntry = null;
    return;
  }
}

toggleChatOverlay() {
  if (iconOverlayEntry == null || windowOverlayEntry == null) {
    showChatIconOverlay();
    showChatWindowOverlay();
  } else {
    hideChatIconOverlay();
    hideChatWindowOverlay();
  }
}

class ChatWindowOverlay extends StatefulWidget {
  final double windowWidth = 250;
  final double windowHeight = 350;

  @override
  State<StatefulWidget> createState() => _ChatWindowOverlayState();
}

class _ChatWindowOverlayState extends State<ChatWindowOverlay> {
  Offset _o = Offset(20, 80);
  bool _keyboardVisible = false;
  double _saveHeight = 0;
  double _lastBottomHeight = 0;

  changeOffset(Offset offset) {
    final size = MediaQuery.of(context).size;
    debugPrint("parent size:$size");
    double x = 0;
    double y = 0;

    if (_o.dx + offset.dx + widget.windowWidth > size.width) {
      x = size.width - widget.windowWidth;
    } else if (_o.dx + offset.dx < 0) {
      x = 0;
    } else {
      x = _o.dx + offset.dx;
    }

    if (_o.dy + offset.dy + widget.windowHeight > size.height) {
      y = size.height - widget.windowHeight;
    } else if (_o.dy + offset.dy < 0) {
      y = 0;
    } else {
      y = _o.dy + offset.dy;
    }
    setState(() {
      _o = Offset(x, y);
    });
  }

  checkScreenSize() {}

  checkKeyboard() {
    final bottomHeight = MediaQuery.of(context).viewInsets.bottom;
    final currentVisible = bottomHeight != 0;

    debugPrint(bottomHeight.toString() + currentVisible.toString());
    // save
    if (!_keyboardVisible && currentVisible) {
      _saveHeight = _o.dy;
      debugPrint("on save $_saveHeight");
    }

    // reset
    if (_lastBottomHeight > 0 && bottomHeight == 0) {
      debugPrint("on reset");
      _o = Offset(_o.dx, _saveHeight);
    }

    // onKeyboardVisible
    if (_keyboardVisible && currentVisible) {
      final sumHeight = bottomHeight + widget.windowHeight;
      final contextHeight = MediaQuery.of(context).size.height;
      debugPrint(
          "prepare update sumHeight:$sumHeight,contextHeight:$contextHeight");
      if (sumHeight + _o.dy > contextHeight) {
        final y = contextHeight - sumHeight;
        debugPrint("on update");
        _o = Offset(_o.dx, y);
      }
    }

    _keyboardVisible = currentVisible;
    _lastBottomHeight = bottomHeight;
  }

  @override
  Widget build(BuildContext context) {
    checkKeyboard();
    checkScreenSize();
    return Positioned(
        top: _o.dy,
        left: _o.dx,
        width: widget.windowWidth,
        height: widget.windowHeight,
        child: Scaffold(
          resizeToAvoidBottomInset: false,
          appBar: CustomAppBar(
            onPanUpdate: (d) => changeOffset(d.delta),
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
          body: chatPage,
        ));
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
