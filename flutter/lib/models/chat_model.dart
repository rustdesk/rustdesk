import 'dart:convert';

import 'package:dash_chat_2/dash_chat_2.dart';
import 'package:flutter/material.dart';

import '../widgets/overlay.dart';
import 'model.dart';

class MessageBody {
  ChatUser chatUser;
  List<ChatMessage> chatMessages;
  MessageBody(this.chatUser, this.chatMessages);

  void insert(ChatMessage cm) {
    this.chatMessages.insert(0, cm);
  }

  void clear() {
    this.chatMessages.clear();
  }
}

class ChatModel with ChangeNotifier {
  static final clientModeID = -1;

  final ChatUser me = ChatUser(
    id: "",
    firstName: "Me",
  );

  late final Map<int, MessageBody> _messages = Map()
    ..[clientModeID] = MessageBody(me, []);

  var _currentID = clientModeID;

  Map<int, MessageBody> get messages => _messages;

  int get currentID => _currentID;

  ChatUser get currentUser {
    final user = messages[currentID]?.chatUser;
    if (user == null) {
      _currentID = clientModeID;
      return me;
    } else {
      return user;
    }
  }

  changeCurrentID(int id) {
    if (_messages.containsKey(id)) {
      _currentID = id;
      notifyListeners();
    } else {
      final client = FFI.serverModel.clients[id];
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

  receive(int id, String text) {
    if (text.isEmpty) return;
    // first message show overlay icon
    if (chatIconOverlayEntry == null) {
      showChatIconOverlay();
    }
    late final chatUser;
    if (id == clientModeID) {
      chatUser = ChatUser(
        firstName: FFI.ffiModel.pi.username,
        id: FFI.getId(),
      );
    } else {
      final client = FFI.serverModel.clients[id];
      if (client == null) {
        return debugPrint("Failed to receive msg,user doesn't exist");
      }
      chatUser = ChatUser(id: client.peerId, firstName: client.name);
    }

    if (!_messages.containsKey(id)) {
      _messages[id] = MessageBody(chatUser, []);
    }
    _messages[id]!.insert(
        ChatMessage(text: text, user: chatUser, createdAt: DateTime.now()));
    _currentID = id;
    notifyListeners();
  }

  send(ChatMessage message) {
    if (message.text.isNotEmpty) {
      _messages[_currentID]?.insert(message);
      if (_currentID == clientModeID) {
        FFI.setByName("chat_client_mode", message.text);
      } else {
        final msg = Map()
          ..["id"] = _currentID
          ..["text"] = message.text;
        FFI.setByName("chat_server_mode", jsonEncode(msg));
      }
    }
    notifyListeners();
  }

  close() {
    hideChatIconOverlay();
    hideChatWindowOverlay();
    notifyListeners();
  }

  resetClientMode() {
    _messages[clientModeID]?.clear();
  }
}
