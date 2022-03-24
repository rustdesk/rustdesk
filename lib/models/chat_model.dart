import 'dart:convert';

import 'package:dash_chat/dash_chat.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/pages/chat_page.dart';

import 'model.dart';

class ChatModel with ChangeNotifier {
  // -1作为客户端模式的id，客户端模式下此id唯一
  // 其它正整数的id，来自被控服务器模式下的其他客户端的id，每个客户端有不同的id
  // 注意 此id和peer_id不同，服务端模式下的id等同于conn的顺序累加id
  static final clientModeID = -1;

  final Map<int, List<ChatMessage>> _messages = Map()..[clientModeID] = [];

  final ChatUser me = ChatUser(
    uid:"",
    name: "me",
    customProperties: Map()..["id"] = clientModeID
  );

  var _currentID = clientModeID;

  get messages => _messages;

  get currentID => _currentID;

  receive(int id, String text) {
    if (text.isEmpty) return;
    // first message show overlay icon
    if (iconOverlayEntry == null) {
      showChatIconOverlay();
    }
    if(!_messages.containsKey(id)){
      _messages[id] = [];
    }
    // TODO  peer info
    _messages[id]?.add(ChatMessage(
        text: text,
        user: ChatUser(
          name: FFI.ffiModel.pi.username,
          uid: FFI.getId(),
        )));
    _currentID = id;
    notifyListeners();
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
  }

  release() {
    hideChatIconOverlay();
    hideChatWindowOverlay();
    _messages.forEach((key, value) => value.clear());
    _messages.clear();
    notifyListeners();
  }
}
