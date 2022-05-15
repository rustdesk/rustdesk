import 'package:dash_chat/dash_chat.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:provider/provider.dart';
import '../models/model.dart';
import 'home_page.dart';

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
          return chatModel.messages.entries.map((entry) {
            final id = entry.key;
            final user = entry.value.chatUser;
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
                    messages:
                        chatModel.messages[chatModel.currentID]?.chatMessages ??
                            [],
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
