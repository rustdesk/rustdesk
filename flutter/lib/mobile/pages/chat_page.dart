import 'package:dash_chat_2/dash_chat_2.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:provider/provider.dart';

import 'home_page.dart';

class ChatPage extends StatelessWidget implements PageShape {
  late final ChatModel chatModel;

  ChatPage({ChatModel? chatModel}) {
    this.chatModel = chatModel ?? gFFI.chatModel;
  }

  @override
  final title = translate("Chat");

  @override
  final icon = Icon(Icons.chat);

  @override
  final appBarActions = [
    PopupMenuButton<int>(
        icon: Icon(Icons.group),
        itemBuilder: (context) {
          // only mobile need [appBarActions], just bind gFFI.chatModel
          final chatModel = gFFI.chatModel;
          return chatModel.messages.entries.map((entry) {
            final id = entry.key;
            final user = entry.value.chatUser;
            return PopupMenuItem<int>(
              child: Text("${user.firstName}   ${user.id}"),
              value: id,
            );
          }).toList();
        },
        onSelected: (id) {
          gFFI.chatModel.changeCurrentID(id);
        })
  ];

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
        value: chatModel,
        child: Container(
            color: Theme.of(context).scaffoldBackgroundColor,
            child: Consumer<ChatModel>(builder: (context, chatModel, child) {
              final currentUser = chatModel.currentUser;
              return Stack(
                children: [
                  LayoutBuilder(builder: (context, constraints) {
                    return DashChat(
                      onSend: (chatMsg) {
                        chatModel.send(chatMsg);
                      },
                      currentUser: chatModel.me,
                      messages: chatModel
                              .messages[chatModel.currentID]?.chatMessages ??
                          [],
                      inputOptions: InputOptions(
                          sendOnEnter: true,
                          inputDecoration: defaultInputDecoration(
                              fillColor: Theme.of(context).backgroundColor),
                          sendButtonBuilder: defaultSendButton(
                              color: Theme.of(context)
                                  .textTheme
                                  .titleLarge!
                                  .color!),
                          inputTextStyle: TextStyle(
                              color: Theme.of(context)
                                  .textTheme
                                  .titleLarge
                                  ?.color)),
                      messageOptions: MessageOptions(
                          showOtherUsersAvatar: false,
                          showTime: true,
                          maxWidth: constraints.maxWidth * 0.7,
                          messageDecorationBuilder: (_, __, ___) =>
                              defaultMessageDecoration(
                                color: MyTheme.accent80,
                                borderTopLeft: 8,
                                borderTopRight: 8,
                                borderBottomRight: 8,
                                borderBottomLeft: 8,
                              )),
                    );
                  }),
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
                                "${currentUser.firstName}   ${currentUser.id}",
                                style: TextStyle(color: MyTheme.accent50),
                              ),
                            ],
                          )),
                ],
              );
            })));
  }
}
