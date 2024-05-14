import 'dart:async';

import 'package:dash_chat_2/dash_chat_2.dart';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:draggable_float_widget/draggable_float_widget.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/mobile/pages/home_page.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:uuid/uuid.dart';
import 'package:window_manager/window_manager.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../consts.dart';
import '../common.dart';
import '../common/widgets/overlay.dart';
import '../main.dart';
import 'model.dart';

class MessageKey {
  final String peerId;
  final int connId;
  bool get isOut => connId == ChatModel.clientModeID;

  MessageKey(this.peerId, this.connId);

  @override
  bool operator ==(other) {
    return other is MessageKey &&
        other.peerId == peerId &&
        other.isOut == isOut;
  }

  @override
  int get hashCode => peerId.hashCode ^ isOut.hashCode;
}

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

  OverlayEntry? chatIconOverlayEntry;
  OverlayEntry? chatWindowOverlayEntry;

  bool isConnManager = false;

  RxBool isWindowFocus = true.obs;
  BlockableOverlayState _blockableOverlayState = BlockableOverlayState();
  final Rx<VoiceCallStatus> _voiceCallStatus = Rx(VoiceCallStatus.notStarted);

  Rx<VoiceCallStatus> get voiceCallStatus => _voiceCallStatus;

  TextEditingController textController = TextEditingController();
  RxInt mobileUnreadSum = 0.obs;
  MessageKey? latestReceivedKey;

  Offset chatWindowPosition = Offset(20, 80);

  void setChatWindowPosition(Offset position) {
    chatWindowPosition = position;
    notifyListeners();
  }

  @override
  void dispose() {
    textController.dispose();
    super.dispose();
  }

  final ChatUser me = ChatUser(
    id: Uuid().v4().toString(),
    firstName: translate("Me"),
  );

  late final Map<MessageKey, MessageBody> _messages = {};

  MessageKey _currentKey = MessageKey('', -2); // -2 is invalid value
  late bool _isShowCMSidePage = false;

  Map<MessageKey, MessageBody> get messages => _messages;

  MessageKey get currentKey => _currentKey;

  bool get isShowCMSidePage => _isShowCMSidePage;

  void setOverlayState(BlockableOverlayState blockableOverlayState) {
    _blockableOverlayState = blockableOverlayState;

    _blockableOverlayState.addMiddleBlockedListener((v) {
      if (!v) {
        isWindowFocus.value = false;
        if (isWindowFocus.value) {
          isWindowFocus.toggle();
        }
      }
    });
  }

  final WeakReference<FFI> parent;

  late final SessionID sessionId;
  late FocusNode inputNode;

  ChatModel(this.parent) {
    sessionId = parent.target!.sessionId;
    inputNode = FocusNode(
      onKey: (_, event) {
        bool isShiftPressed = event.isKeyPressed(LogicalKeyboardKey.shiftLeft);
        bool isEnterPressed = event.isKeyPressed(LogicalKeyboardKey.enter);

        // don't send empty messages
        if (isEnterPressed && isEnterPressed && textController.text.isEmpty) {
          return KeyEventResult.handled;
        }

        if (isEnterPressed && !isShiftPressed) {
          final ChatMessage message = ChatMessage(
            text: textController.text,
            user: me,
            createdAt: DateTime.now(),
          );
          send(message);
          textController.clear();
          return KeyEventResult.handled;
        }

        return KeyEventResult.ignored;
      },
    );
  }

  ChatUser? get currentUser => _messages[_currentKey]?.chatUser;

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

    final overlayState = _blockableOverlayState.state;
    if (overlayState == null) return;

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
          backgroundColor: Theme.of(context).colorScheme.primary,
          child: SvgPicture.asset('assets/chat2.svg'),
        ),
      );
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

  showChatWindowOverlay({Offset? chatInitPos}) {
    if (chatWindowOverlayEntry != null) return;
    isWindowFocus.value = true;
    _blockableOverlayState.setMiddleBlocked(true);

    final overlayState = _blockableOverlayState.state;
    if (overlayState == null) return;
    if (isMobile &&
        !gFFI.chatModel.currentKey.isOut && // not in remote page
        gFFI.chatModel.latestReceivedKey != null) {
      gFFI.chatModel.changeCurrentKey(gFFI.chatModel.latestReceivedKey!);
      gFFI.chatModel.mobileClearClientUnread(gFFI.chatModel.currentKey.connId);
    }
    final overlay = OverlayEntry(builder: (context) {
      return Listener(
          onPointerDown: (_) {
            if (!isWindowFocus.value) {
              isWindowFocus.value = true;
              _blockableOverlayState.setMiddleBlocked(true);
            }
          },
          child: DraggableChatWindow(
              position: chatInitPos ?? chatWindowPosition,
              width: 250,
              height: 350,
              chatModel: this));
    });
    overlayState.insert(overlay);
    chatWindowOverlayEntry = overlay;
    requestChatInputFocus();
  }

  hideChatWindowOverlay() {
    if (chatWindowOverlayEntry != null) {
      _blockableOverlayState.setMiddleBlocked(false);
      chatWindowOverlayEntry!.remove();
      chatWindowOverlayEntry = null;
      return;
    }
  }

  _isChatOverlayHide() => ((!isDesktop && chatIconOverlayEntry == null) ||
      chatWindowOverlayEntry == null);

  toggleChatOverlay({Offset? chatInitPos}) {
    if (_isChatOverlayHide()) {
      gFFI.invokeMethod("enable_soft_keyboard", true);
      if (!isDesktop) {
        showChatIconOverlay();
      }
      showChatWindowOverlay(chatInitPos: chatInitPos);
    } else {
      hideChatIconOverlay();
      hideChatWindowOverlay();
    }
  }

  hideChatOverlay() {
    if (!_isChatOverlayHide()) {
      hideChatIconOverlay();
      hideChatWindowOverlay();
    }
  }

  showChatPage(MessageKey key) async {
    if (isDesktop) {
      if (isConnManager) {
        if (!_isShowCMSidePage) {
          await toggleCMChatPage(key);
        }
      } else {
        if (_isChatOverlayHide()) {
          await toggleChatOverlay();
        }
      }
    } else {
      if (key.connId == clientModeID) {
        if (_isChatOverlayHide()) {
          await toggleChatOverlay();
        }
      }
    }
  }

  toggleCMChatPage(MessageKey key) async {
    if (gFFI.chatModel.currentKey != key) {
      gFFI.chatModel.changeCurrentKey(key);
    }
    await toggleCMSidePage();
  }

  toggleCMFilePage() async {
    await toggleCMSidePage();
  }

  var _togglingCMSidePage = false; // protect order for await
  toggleCMSidePage() async {
    if (_togglingCMSidePage) return false;
    _togglingCMSidePage = true;
    if (_isShowCMSidePage) {
      _isShowCMSidePage = !_isShowCMSidePage;
      notifyListeners();
      await windowManager.show();
      await windowManager.setSizeAlignment(
          kConnectionManagerWindowSizeClosedChat, Alignment.topRight);
    } else {
      final currentSelectedTab =
          gFFI.serverModel.tabController.state.value.selectedTabInfo;
      final client = parent.target?.serverModel.clients.firstWhereOrNull(
          (client) => client.id.toString() == currentSelectedTab.key);
      if (client != null) {
        client.unreadChatMessageCount.value = 0;
      }
      requestChatInputFocus();
      await windowManager.show();
      await windowManager.setSizeAlignment(
          kConnectionManagerWindowSizeOpenChat, Alignment.topRight);
      _isShowCMSidePage = !_isShowCMSidePage;
      notifyListeners();
    }
    _togglingCMSidePage = false;
  }

  changeCurrentKey(MessageKey key) {
    updateConnIdOfKey(key);
    String? peerName;
    if (key.connId == clientModeID) {
      peerName = parent.target?.ffiModel.pi.username;
    } else {
      peerName = parent.target?.serverModel.clients
          .firstWhereOrNull((client) => client.peerId == key.peerId)
          ?.name;
    }
    if (!_messages.containsKey(key)) {
      final chatUser = ChatUser(
        id: key.peerId,
        firstName: peerName,
      );
      _messages[key] = MessageBody(chatUser, []);
    } else {
      if (peerName != null && peerName.isNotEmpty) {
        _messages[key]?.chatUser.firstName = peerName;
      }
    }
    _currentKey = key;
    notifyListeners();
    mobileClearClientUnread(key.connId);
  }

  receive(int id, String text) async {
    final session = parent.target;
    if (session == null) {
      debugPrint("Failed to receive msg, session state is null");
      return;
    }
    if (text.isEmpty) return;
    if (desktopType == DesktopType.cm) {
      await showCmWindow();
    }
    String? peerId;
    if (id == clientModeID) {
      peerId = session.id;
    } else {
      peerId = session.serverModel.clients
          .firstWhereOrNull((e) => e.id == id)
          ?.peerId;
    }
    if (peerId == null) {
      debugPrint("Failed to receive msg, peerId is null");
      return;
    }

    final messagekey = MessageKey(peerId, id);

    // mobile: first message show overlay icon
    if (!isDesktop && chatIconOverlayEntry == null) {
      showChatIconOverlay();
    }
    // show chat page
    await showChatPage(messagekey);
    late final ChatUser chatUser;
    if (id == clientModeID) {
      chatUser = ChatUser(
        firstName: session.ffiModel.pi.username,
        id: peerId,
      );

      if (isDesktop) {
        if (Get.isRegistered<DesktopTabController>()) {
          DesktopTabController tabController = Get.find<DesktopTabController>();
          var index = tabController.state.value.tabs
              .indexWhere((e) => e.key == session.id);
          final notSelected =
              index >= 0 && tabController.state.value.selected != index;
          // minisized: top and switch tab
          // not minisized: add count
          if (await WindowController.fromWindowId(stateGlobal.windowId)
              .isMinimized()) {
            windowOnTop(stateGlobal.windowId);
            if (notSelected) {
              tabController.jumpTo(index);
            }
          } else {
            if (notSelected) {
              UnreadChatCountState.find(peerId).value += 1;
            }
          }
        }
      }
    } else {
      final client = session.serverModel.clients
          .firstWhereOrNull((client) => client.id == id);
      if (client == null) {
        debugPrint("Failed to receive msg, client is null");
        return;
      }
      if (isDesktop) {
        windowOnTop(null);
        // disable auto jumpTo other tab when hasFocus, and mark unread message
        final currentSelectedTab =
            session.serverModel.tabController.state.value.selectedTabInfo;
        if (currentSelectedTab.key != id.toString() && inputNode.hasFocus) {
          client.unreadChatMessageCount.value += 1;
        } else {
          parent.target?.serverModel.jumpTo(id);
        }
      } else {
        if (HomePage.homeKey.currentState?.isChatPageCurrentTab != true ||
            _currentKey != messagekey) {
          client.unreadChatMessageCount.value += 1;
          mobileUpdateUnreadSum();
        }
      }
      chatUser = ChatUser(id: client.peerId, firstName: client.name);
    }
    insertMessage(messagekey,
        ChatMessage(text: text, user: chatUser, createdAt: DateTime.now()));
    if (id == clientModeID || _currentKey.peerId.isEmpty) {
      // client or invalid
      _currentKey = messagekey;
      mobileClearClientUnread(messagekey.connId);
    }
    latestReceivedKey = messagekey;
    notifyListeners();
  }

  send(ChatMessage message) {
    String trimmedText = message.text.trim();
    if (trimmedText.isEmpty) {
      return;
    }
    message.text = trimmedText;
    insertMessage(_currentKey, message);
    if (_currentKey.connId == clientModeID && parent.target != null) {
      bind.sessionSendChat(sessionId: sessionId, text: message.text);
    } else {
      bind.cmSendChat(connId: _currentKey.connId, msg: message.text);
    }

    notifyListeners();
    inputNode.requestFocus();
  }

  insertMessage(MessageKey key, ChatMessage message) {
    updateConnIdOfKey(key);
    if (!_messages.containsKey(key)) {
      _messages[key] = MessageBody(message.user, []);
    }
    _messages[key]?.insert(message);
  }

  updateConnIdOfKey(MessageKey key) {
    if (_messages.keys
            .toList()
            .firstWhereOrNull((e) => e == key && e.connId != key.connId) !=
        null) {
      final value = _messages.remove(key);
      if (value != null) {
        _messages[key] = value;
      }
    }
    if (_currentKey == key || _currentKey.peerId.isEmpty) {
      _currentKey = key; // hash != assign
    }
  }

  void mobileUpdateUnreadSum() {
    if (!isMobile) return;
    var sum = 0;
    parent.target?.serverModel.clients
        .map((e) => sum += e.unreadChatMessageCount.value)
        .toList();
    Future.delayed(Duration.zero, () {
      mobileUnreadSum.value = sum;
    });
  }

  void mobileClearClientUnread(int id) {
    if (!isMobile) return;
    final client = parent.target?.serverModel.clients
        .firstWhereOrNull((client) => client.id == id);
    if (client != null) {
      Future.delayed(Duration.zero, () {
        client.unreadChatMessageCount.value = 0;
        mobileUpdateUnreadSum();
      });
    }
  }

  close() {
    hideChatIconOverlay();
    hideChatWindowOverlay();
    notifyListeners();
  }

  resetClientMode() {
    _messages[clientModeID]?.clear();
  }

  void requestChatInputFocus() {
    Timer(Duration(milliseconds: 100), () {
      if (inputNode.hasListeners && inputNode.canRequestFocus) {
        inputNode.requestFocus();
      }
    });
  }

  void onVoiceCallWaiting() {
    _voiceCallStatus.value = VoiceCallStatus.waitingForResponse;
  }

  void onVoiceCallStarted() {
    _voiceCallStatus.value = VoiceCallStatus.connected;
    if (isAndroid) {
      parent.target?.invokeMethod("on_voice_call_started");
    }
  }

  void onVoiceCallClosed(String reason) {
    _voiceCallStatus.value = VoiceCallStatus.notStarted;
    if (isAndroid) {
      parent.target?.invokeMethod("on_voice_call_closed");
    }
  }

  void onVoiceCallIncoming() {
    if (isConnManager) {
      _voiceCallStatus.value = VoiceCallStatus.incoming;
    }
  }

  void closeVoiceCall() {
    bind.sessionCloseVoiceCall(sessionId: sessionId);
  }
}

enum VoiceCallStatus {
  notStarted,
  waitingForResponse,
  connected,
  // Connection manager only.
  incoming
}
