import 'package:dash_chat/dash_chat.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/pages/chat_page.dart';

import 'model.dart';
import 'native_model.dart';

class ChatModel with ChangeNotifier {
  final List<ChatMessage> _messages = [];

  final ChatUser me = ChatUser(
    name:"me",
  );

  get messages => _messages;

  receive(String text){
    if (text.isEmpty) return;
    // first message show overlay icon
    if (iconOverlayEntry == null){
      showChatIconOverlay();
    }
    _messages.add(ChatMessage(text: text, user: ChatUser(
      name:FFI.ffiModel.pi.username,
      uid: FFI.getId(),
    )));
    notifyListeners();
  }

  send(ChatMessage message){
    _messages.add(message);
    if(message.text != null && message.text!.isNotEmpty){
      PlatformFFI.setByName("chat",message.text!);
    }
    notifyListeners();
  }

  release(){
    hideChatIconOverlay();
    hideChatWindowOverlay();
    _messages.clear();
    notifyListeners();
  }
}