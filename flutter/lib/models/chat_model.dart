import 'package:dash_chat_2/dash_chat_2.dart';
import 'package:draggable_float_widget/draggable_float_widget.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:window_manager/window_manager.dart';

import '../common.dart';
import '../common/widgets/overlay.dart';
import 'model.dart';

class MessageBody {
  ChatUser chatUser;
  List<ChatMessage> chatMessages;
  MessageBody(this.chatUser, this.chatMessages);

  void insert(ChatMessage cm) {
    chatMessages.insert(0, cm);
  }

  void clear() {
    chatMessages.clear();
  }
}

class ChatModel with ChangeNotifier {
  static final clientModeID = -1;

  /// _overlayState:
  /// Desktop: store session overlay by using [setOverlayState].
  /// Mobile: always null, use global overlay.
  /// see [_getOverlayState] in [showChatIconOverlay] or [showChatWindowOverlay]
  OverlayState? _overlayState;
  OverlayEntry? chatIconOverlayEntry;
  OverlayEntry? chatWindowOverlayEntry;
  bool _showOnWindowRestore = false;
  bool _isWindowMinimized = false;
  bool isConnManager = false;

  final ChatUser me = ChatUser(
    id: "",
    firstName: "Me",
  );

  late final Map<int, MessageBody> _messages = {}..[clientModeID] =
      MessageBody(me, []);

  var _currentID = clientModeID;
  late bool _isShowCMChatPage = false;

  Map<int, MessageBody> get messages => _messages;

  int get currentID => _currentID;

  bool get isShowCMChatPage => _isShowCMChatPage;

  final WeakReference<FFI> parent;

  ChatModel(this.parent);

  FocusNode inputNode = FocusNode();

  ChatUser get currentUser {
    final user = messages[currentID]?.chatUser;
    if (user == null) {
      _currentID = clientModeID;
      return me;
    } else {
      return user;
    }
  }

  setWindowMinimized(bool v) {
    _isWindowMinimized = v;
    if (!_isWindowMinimized && _showOnWindowRestore) {
      _showOnWindowRestore = false;
    }
  }

  setOverlayState(OverlayState? os) {
    _overlayState = os;
  }

  OverlayState? _getOverlayState() {
    if (_overlayState == null) {
      if (globalKey.currentState == null ||
          globalKey.currentState!.overlay == null) return null;
      return globalKey.currentState!.overlay;
    } else {
      return _overlayState;
    }
  }

  showChatIconOverlay({Offset offset = const Offset(200, 50)}) {
    if (chatIconOverlayEntry != null) {
      chatIconOverlayEntry!.remove();
    }
    // mobile check navigationBar
    final bar = navigationBarKey.currentWidget;
    if (bar != null) {
      if ((bar as BottomNavigationBar).currentIndex == 1) {
        return;
      }
    }

    final overlayState = _getOverlayState();
    if (overlayState == null) return;

    final overlay = OverlayEntry(builder: (context) {
      return DraggableFloatWidget(
          config: DraggableFloatWidgetBaseConfig(
            initPositionYInTop: false,
            initPositionYMarginBorder: 100,
            borderTopContainTopBar: true,
            appBarHeight: 0,
          ),
          child: FloatingActionButton(
              onPressed: () {
                if (chatWindowOverlayEntry == null) {
                  showChatWindowOverlay();
                } else {
                  hideChatWindowOverlay();
                }
              },
              backgroundColor: Theme.of(context).colorScheme.primary,
              child: Icon(Icons.message)));
    });
    overlayState.insert(overlay);
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
    final overlayState = _getOverlayState();
    if (overlayState == null) return;
    final overlay = OverlayEntry(builder: (context) {
      return DraggableChatWindow(
          position: const Offset(20, 80),
          width: 250,
          height: 350,
          chatModel: this);
    });
    overlayState.insert(overlay);
    chatWindowOverlayEntry = overlay;
  }

  hideChatWindowOverlay() {
    if (chatWindowOverlayEntry != null) {
      chatWindowOverlayEntry!.remove();
      chatWindowOverlayEntry = null;
      return;
    }
  }

  _isChatOverlayHide() => ((!isDesktop && chatIconOverlayEntry == null) ||
      chatWindowOverlayEntry == null);

  toggleChatOverlay() {
    if (_isChatOverlayHide()) {
      gFFI.invokeMethod("enable_soft_keyboard", true);
      if (!isDesktop) {
        showChatIconOverlay();
      }
      showChatWindowOverlay();
    } else {
      hideChatIconOverlay();
      hideChatWindowOverlay();
    }
  }

  showChatPage(int id) async {
    if (isConnManager) {
      if (!_isShowCMChatPage) {
        await toggleCMChatPage(id);
      }
    } else {
      if (_isChatOverlayHide()) {
        await toggleChatOverlay();
      }
    }
  }

  toggleCMChatPage(int id) async {
    if (gFFI.chatModel.currentID != id) {
      gFFI.chatModel.changeCurrentID(id);
    }
    if (_isShowCMChatPage) {
      _isShowCMChatPage = !_isShowCMChatPage;
      notifyListeners();
      await windowManager.setSizeAlignment(Size(300, 400), Alignment.topRight);
    } else {
      await windowManager.setSizeAlignment(Size(600, 400), Alignment.topRight);
      _isShowCMChatPage = !_isShowCMChatPage;
      notifyListeners();
    }
  }

  changeCurrentID(int id) {
    if (_messages.containsKey(id)) {
      _currentID = id;
      notifyListeners();
    } else {
      final client = parent.target?.serverModel.clients
          .firstWhere((client) => client.id == id);
      if (client == null) {
        return debugPrint(
            "Failed to changeCurrentID,remote user doesn't exist");
      }
      final chatUser = ChatUser(
        id: client.peerId,
        firstName: client.name,
      );
      _messages[id] = MessageBody(chatUser, []);
      _currentID = id;
      notifyListeners();
    }
  }

  receive(int id, String text) async {
    final session = parent.target;
    if (session == null) {
      debugPrint("Failed to receive msg, session state is null");
      return;
    }
    if (text.isEmpty) return;
    // mobile: first message show overlay icon
    if (!isDesktop && chatIconOverlayEntry == null) {
      showChatIconOverlay();
    }
    // show chat page
    await showChatPage(id);

    int toId = currentID;

    late final ChatUser chatUser;
    if (id == clientModeID) {
      chatUser = ChatUser(
        firstName: session.ffiModel.pi.username,
        id: session.id,
      );
      toId = id;
    } else {
      final client =
          session.serverModel.clients.firstWhere((client) => client.id == id);
      if (isDesktop) {
        window_on_top(null);
        // disable auto jumpTo other tab when hasFocus, and mark unread message
        final currentSelectedTab =
            session.serverModel.tabController.state.value.selectedTabInfo;
        if (currentSelectedTab.key != id.toString() && inputNode.hasFocus) {
          client.hasUnreadChatMessage.value = true;
        } else {
          parent.target?.serverModel.jumpTo(id);
          toId = id;
        }
      } else {
        toId = id;
      }
      chatUser = ChatUser(id: client.peerId, firstName: client.name);
    }

    if (!_messages.containsKey(id)) {
      _messages[id] = MessageBody(chatUser, []);
    }
    _messages[id]!.insert(
        ChatMessage(text: text, user: chatUser, createdAt: DateTime.now()));
    _currentID = toId;
    notifyListeners();
  }

  send(ChatMessage message) {
    if (message.text.isNotEmpty) {
      _messages[_currentID]?.insert(message);
      if (_currentID == clientModeID) {
        if (parent.target != null) {
          bind.sessionSendChat(id: parent.target!.id, text: message.text);
        }
      } else {
        bind.cmSendChat(connId: _currentID, msg: message.text);
      }
    }
    notifyListeners();
  }

  close() {
    hideChatIconOverlay();
    hideChatWindowOverlay();
    _overlayState = null;
    notifyListeners();
  }

  resetClientMode() {
    _messages[clientModeID]?.clear();
  }
}
