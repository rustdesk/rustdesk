import 'dart:convert';

import 'package:dash_chat/dash_chat.dart';
import 'package:flutter/material.dart';

import '../widgets/overlay.dart';
import 'model.dart';

class ChatModel with ChangeNotifier {
  static final clientModeID = -1;

  final Map<int, List<ChatMessage>> _messages = Map()..[clientModeID] = [];

  final ChatUser me = ChatUser(
    uid: "",
    name: "Me",
  );

  final _scroller = ScrollController();

  var _currentID = clientModeID;

  ScrollController get scroller => _scroller;

  Map<int, List<ChatMessage>> get messages => _messages;

  int get currentID => _currentID;

  ChatUser get currentUser =>
      FFI.serverModel.clients[_currentID]?.chatUser ?? me;

  changeCurrentID(int id) {
    if (_messages.containsKey(id)) {
      _currentID = id;
      notifyListeners();
    } else {
      final chatUser = FFI.serverModel.clients[id]?.chatUser;
      if (chatUser == null) {
        return debugPrint(
            "Failed to changeCurrentID,remote user doesn't exist");
      }
      _messages[id] = [];
      _currentID = id;
    }
  }

  receive(int id, String text) {
    if (text.isEmpty) return;
    // first message show overlay icon
    if (chatIconOverlayEntry == null) {
      showChatIconOverlay();
    }
    late final chatUser;
    if (id == clientModeID) {
      chatUser = ChatUser(
        name: FFI.ffiModel.pi.username,
        uid: FFI.getId(),
      );
    } else {
      chatUser = FFI.serverModel.clients[id]?.chatUser;
    }
    if (chatUser == null) {
      return debugPrint("Failed to receive msg,user doesn't exist");
    }
    if (!_messages.containsKey(id)) {
      _messages[id] = [];
    }
    _messages[id]!.add(ChatMessage(text: text, user: chatUser));
    _currentID = id;
    notifyListeners();
    scrollToBottom();
  }

  scrollToBottom() {
    Future.delayed(Duration(milliseconds: 500), () {
      _scroller.animateTo(_scroller.position.maxScrollExtent,
          duration: Duration(milliseconds: 200),
          curve: Curves.fastLinearToSlowEaseIn);
    });
  }

  send(ChatMessage message) {
    if (message.text != null && message.text!.isNotEmpty) {
      _messages[_currentID]?.add(message);
      if (_currentID == clientModeID) {
        FFI.setByName("chat_client_mode", message.text!);
      } else {
        final msg = Map()
          ..["id"] = _currentID
          ..["text"] = message.text!;
        FFI.setByName("chat_server_mode", jsonEncode(msg));
      }
    }
    notifyListeners();
    scrollToBottom();
  }

  close() {
    hideChatIconOverlay();
    hideChatWindowOverlay();
    notifyListeners();
  }
}
