import 'package:dash_chat/dash_chat.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'home_page.dart';


class ChatPage extends StatelessWidget implements PageShape {

  final FocusNode _focusNode = FocusNode();

  @override
  final title = "Chat";

  @override
  final icon = Icon(Icons.chat);

  @override
  final appBarActions = [];

  @override
  Widget build(BuildContext context) {
    return Container(
      color: MyTheme.darkGray,
      child: DashChat(
        focusNode: _focusNode,
        onSend: (ChatMessage) {},
        user: ChatUser(uid: "111", name: "Bob"),
        messages: [
          ChatMessage(
              text: "hello", user: ChatUser(uid: "111", name: "Bob")),
          ChatMessage(
              text: "hi", user: ChatUser(uid: "222", name: "Alice")),
          ChatMessage(
              text: "hello", user: ChatUser(uid: "111", name: "Bob")),
          ChatMessage(
              text: "hi", user: ChatUser(uid: "222", name: "Alice")),
          ChatMessage(
              text: "hello", user: ChatUser(uid: "111", name: "Bob")),
          ChatMessage(
              text: "hi", user: ChatUser(uid: "222", name: "Alice")),
          ChatMessage(
              text: "hello", user: ChatUser(uid: "111", name: "Bob")),
          ChatMessage(
              text: "hi", user: ChatUser(uid: "222", name: "Alice")),
        ],
      ),
    );
  }
}