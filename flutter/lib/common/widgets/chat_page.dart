import 'package:dash_chat_2/dash_chat_2.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';

import '../../mobile/pages/home_page.dart';

enum ChatPageType {
  mobileMain,
}

class ChatPage extends StatelessWidget implements PageShape {
  late final ChatModel chatModel;
  final ChatPageType? type;

  ChatPage({ChatModel? chatModel, this.type}) {
    this.chatModel = chatModel ?? gFFI.chatModel;
  }

  @override
  final title = translate("Chat");

  @override
  final icon = Icon(Icons.chat);

  @override
  final appBarActions = [
    PopupMenuButton<MessageKey>(
        tooltip: "",
        icon: Stack(
          children: [
            Icon(Icons.group),
            Positioned(
                top: 0,
                right: 0,
                child: unreadMessageCountBuilder(gFFI.chatModel.mobileUnreadSum,
                    marginLeft: 0, size: 12, fontSize: 8))
          ],
        ),
        itemBuilder: (context) {
          // only mobile need [appBarActions], just bind gFFI.chatModel
          final chatModel = gFFI.chatModel;
          return chatModel.messages.entries.map((entry) {
            final id = entry.key;
            final user = entry.value.chatUser;
            final client = gFFI.serverModel.clients
                .firstWhereOrNull((e) => e.id == id.connId);
            return PopupMenuItem<MessageKey>(
              child: Row(
                children: [
                  Icon(
                          id.isOut
                              ? Icons.call_made_rounded
                              : Icons.call_received_rounded,
                          color: MyTheme.accent)
                      .marginOnly(right: 6),
                  Text("${user.firstName}   ${user.id}"),
                  if (client != null)
                    unreadMessageCountBuilder(client.unreadChatMessageCount)
                ],
              ),
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
        child: Consumer<ChatModel>(
          builder: (context, chatModel, child) {
            final currentUser = chatModel.currentUser;
            return Stack(
              children: [
                LayoutBuilder(builder: (context, constraints) {
                  final chat = DashChat(
                    onSend: chatModel.send,
                    currentUser: chatModel.me,
                    messages: chatModel
                            .messages[chatModel.currentKey]?.chatMessages ??
                        [],
                    readOnly: type == ChatPageType.mobileMain &&
                        (chatModel.currentKey.connId ==
                                ChatModel.clientModeID ||
                            gFFI.serverModel.clients.every(
                                (e) => e.id != chatModel.currentKey.connId)),
                    inputOptions: InputOptions(
                      focusNode: chatModel.inputNode,
                      textController: chatModel.textController,
                      inputTextStyle: TextStyle(
                          fontSize: 14,
                          color: Theme.of(context).textTheme.titleLarge?.color),
                      inputDecoration: InputDecoration(
                        isDense: true,
                        hintText: translate('Write a message'),
                        filled: true,
                        fillColor: Theme.of(context).colorScheme.background,
                        contentPadding: EdgeInsets.all(10),
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(10.0),
                          borderSide: const BorderSide(
                            width: 1,
                            style: BorderStyle.solid,
                          ),
                        ),
                      ),
                      sendButtonBuilder: defaultSendButton(
                        padding:
                            EdgeInsets.symmetric(horizontal: 6, vertical: 0),
                        color: MyTheme.accent,
                        icon: Icons.send_rounded,
                      ),
                    ),
                    messageOptions: MessageOptions(
                      showOtherUsersAvatar: false,
                      showOtherUsersName: false,
                      textColor: Colors.white,
                      maxWidth: constraints.maxWidth * 0.7,
                      messageTextBuilder: (message, _, __) {
                        final isOwnMessage = message.user.id.isBlank!;
                        return Column(
                          crossAxisAlignment: isOwnMessage
                              ? CrossAxisAlignment.end
                              : CrossAxisAlignment.start,
                          children: <Widget>[
                            Text(message.text,
                                style: TextStyle(color: Colors.white)),
                            Text(
                              "${message.createdAt.hour}:${message.createdAt.minute.toString().padLeft(2, '0')}",
                              style: TextStyle(
                                color: Colors.white,
                                fontSize: 8,
                              ),
                            ).marginOnly(top: 3),
                          ],
                        );
                      },
                      messageDecorationBuilder:
                          (message, previousMessage, nextMessage) {
                        final isOwnMessage = message.user.id.isBlank!;
                        return defaultMessageDecoration(
                          color:
                              isOwnMessage ? MyTheme.accent : Colors.blueGrey,
                          borderTopLeft: 8,
                          borderTopRight: 8,
                          borderBottomRight: isOwnMessage ? 2 : 8,
                          borderBottomLeft: isOwnMessage ? 8 : 2,
                        );
                      },
                    ),
                  );
                  return SelectionArea(child: chat);
                }),
                desktopType == DesktopType.cm ||
                        type != ChatPageType.mobileMain ||
                        currentUser == null
                    ? SizedBox.shrink()
                    : Padding(
                        padding: EdgeInsets.all(12),
                        child: Row(
                          children: [
                            Icon(
                                chatModel.currentKey.isOut
                                    ? Icons.call_made_rounded
                                    : Icons.call_received_rounded,
                                color: MyTheme.accent),
                            Icon(Icons.account_circle, color: MyTheme.accent80),
                            SizedBox(width: 5),
                            Text(
                              "${currentUser.firstName}   ${currentUser.id}",
                              style: TextStyle(color: MyTheme.accent),
                            ),
                          ],
                        ),
                      ),
              ],
            ).paddingOnly(bottom: 8);
          },
        ),
      ),
    );
  }
}
