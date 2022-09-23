// main window right pane

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/address_book.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher_string.dart';

import '../../common.dart';
import '../../common/formatter/id_formatter.dart';
import '../../common/widgets/peer_tab_page.dart';
import '../../common/widgets/peers_view.dart';
import '../../models/platform_model.dart';

/// Connection page for connecting to a remote peer.
class ConnectionPage extends StatefulWidget {
  const ConnectionPage({Key? key}) : super(key: key);

  @override
  State<ConnectionPage> createState() => _ConnectionPageState();
}

/// State for the connection page.
class _ConnectionPageState extends State<ConnectionPage> {
  /// Controller for the id input bar.
  final _idController = IDTextEditingController();

  Timer? _updateTimer;

  @override
  void initState() {
    super.initState();
    if (_idController.text.isEmpty) {
      () async {
        final lastRemoteId = await bind.mainGetLastRemoteId();
        if (lastRemoteId != _idController.id) {
          setState(() {
            _idController.id = lastRemoteId;
          });
        }
      }();
    }
    _updateTimer = Timer.periodic(Duration(seconds: 1), (timer) {
      updateStatus();
    });
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      child: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          mainAxisSize: MainAxisSize.max,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Expanded(
              child: Column(
                children: [
                  Row(
                    children: [
                      _buildRemoteIDTextField(context),
                    ],
                  ).marginOnly(top: 22),
                  SizedBox(height: 12),
                  Divider(),
                  Expanded(
                      child: PeerTabPage(
                    tabs: [
                      translate('Recent Sessions'),
                      translate('Favorites'),
                      translate('Discovered'),
                      translate('Address Book')
                    ],
                    children: [
                      RecentPeersView(
                        menuPadding: EdgeInsets.only(left: 12.0, right: 3.0),
                      ),
                      FavoritePeersView(
                        menuPadding: EdgeInsets.only(left: 12.0, right: 3.0),
                      ),
                      DiscoveredPeersView(
                        menuPadding: EdgeInsets.only(left: 12.0, right: 3.0),
                      ),
                      const AddressBook(
                        menuPadding: EdgeInsets.only(left: 12.0, right: 3.0),
                      ),
                    ],
                  )),
                ],
              ).marginSymmetric(horizontal: 22),
            ),
            const Divider(),
            SizedBox(child: Obx(() => buildStatus()))
                .paddingOnly(bottom: 12, top: 6),
          ]),
    );
  }

  /// Callback for the connect button.
  /// Connects to the selected peer.
  void onConnect({bool isFileTransfer = false}) {
    final id = _idController.id;
    connect(context, id, isFileTransfer: isFileTransfer);
  }

  /// UI for the remote ID TextField.
  /// Search for a peer and connect to it if the id exists.
  Widget _buildRemoteIDTextField(BuildContext context) {
    RxBool ftHover = false.obs;
    RxBool ftPressed = false.obs;
    RxBool connHover = false.obs;
    RxBool connPressed = false.obs;
    RxBool inputFocused = false.obs;
    FocusNode focusNode = FocusNode();
    focusNode.addListener(() {
      inputFocused.value = focusNode.hasFocus;
    });
    var w = Container(
      width: 320 + 20 * 2,
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 22),
      decoration: BoxDecoration(
        color: MyTheme.color(context).bg,
        borderRadius: const BorderRadius.all(Radius.circular(13)),
      ),
      child: Ink(
        child: Column(
          children: [
            Row(
              children: [
                Text(
                  translate('Control Remote Desktop'),
                  style: const TextStyle(fontSize: 19, height: 1),
                ),
              ],
            ).marginOnly(bottom: 15),
            Row(
              children: [
                Expanded(
                  child: Obx(
                    () => TextField(
                      autocorrect: false,
                      enableSuggestions: false,
                      keyboardType: TextInputType.visiblePassword,
                      focusNode: focusNode,
                      style: const TextStyle(
                        fontFamily: 'WorkSans',
                        fontSize: 22,
                        height: 1,
                      ),
                      maxLines: 1,
                      cursorColor: MyTheme.color(context).text!,
                      decoration: InputDecoration(
                          hintText: inputFocused.value
                              ? null
                              : translate('Enter Remote ID'),
                          hintStyle: TextStyle(
                              color: MyTheme.color(context).placeholder),
                          border: OutlineInputBorder(
                              borderRadius: BorderRadius.zero,
                              borderSide: BorderSide(
                                  color: MyTheme.color(context).border!)),
                          enabledBorder: OutlineInputBorder(
                              borderRadius: BorderRadius.zero,
                              borderSide: BorderSide(
                                  color: MyTheme.color(context).border!)),
                          focusedBorder: const OutlineInputBorder(
                            borderRadius: BorderRadius.zero,
                            borderSide:
                                BorderSide(color: MyTheme.button, width: 3),
                          ),
                          isDense: true,
                          contentPadding: const EdgeInsets.symmetric(
                              horizontal: 10, vertical: 12)),
                      controller: _idController,
                      inputFormatters: [IDTextInputFormatter()],
                      onSubmitted: (s) {
                        onConnect();
                      },
                    ),
                  ),
                ),
              ],
            ),
            Padding(
              padding: const EdgeInsets.only(top: 13.0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  Obx(() => InkWell(
                        onTapDown: (_) => ftPressed.value = true,
                        onTapUp: (_) => ftPressed.value = false,
                        onTapCancel: () => ftPressed.value = false,
                        onHover: (value) => ftHover.value = value,
                        onTap: () {
                          onConnect(isFileTransfer: true);
                        },
                        child: Container(
                          height: 27,
                          alignment: Alignment.center,
                          decoration: BoxDecoration(
                            color: ftPressed.value
                                ? MyTheme.accent
                                : Colors.transparent,
                            border: Border.all(
                              color: ftPressed.value
                                  ? MyTheme.accent
                                  : ftHover.value
                                      ? MyTheme.hoverBorder
                                      : MyTheme.border,
                            ),
                            borderRadius: BorderRadius.circular(5),
                          ),
                          child: Text(
                            translate(
                              "Transfer File",
                            ),
                            style: TextStyle(
                                fontSize: 12,
                                color: ftPressed.value
                                    ? MyTheme.color(context).bg
                                    : MyTheme.color(context).text),
                          ).marginSymmetric(horizontal: 12),
                        ),
                      )),
                  const SizedBox(
                    width: 17,
                  ),
                  Obx(
                    () => InkWell(
                      onTapDown: (_) => connPressed.value = true,
                      onTapUp: (_) => connPressed.value = false,
                      onTapCancel: () => connPressed.value = false,
                      onHover: (value) => connHover.value = value,
                      onTap: onConnect,
                      child: ConstrainedBox(
                          constraints: BoxConstraints(
                            minWidth: 80.0,
                          ),
                          child: Container(
                            height: 27,
                            decoration: BoxDecoration(
                              color: connPressed.value
                                  ? MyTheme.accent
                                  : MyTheme.button,
                              border: Border.all(
                                color: connPressed.value
                                    ? MyTheme.accent
                                    : connHover.value
                                        ? MyTheme.hoverBorder
                                        : MyTheme.button,
                              ),
                              borderRadius: BorderRadius.circular(5),
                            ),
                            child: Center(
                              child: Text(
                                translate(
                                  "Connect",
                                ),
                                style: TextStyle(
                                    fontSize: 12,
                                    color: MyTheme.color(context).bg),
                              ),
                            ).marginSymmetric(horizontal: 12),
                          )),
                    ),
                  ),
                ],
              ),
            )
          ],
        ),
      ),
    );
    return Center(
        child: Container(
            constraints: const BoxConstraints(maxWidth: 600), child: w));
  }

  @override
  void dispose() {
    _idController.dispose();
    _updateTimer?.cancel();
    super.dispose();
  }

  var svcStopped = false.obs;
  var svcStatusCode = 0.obs;
  var svcIsUsingPublicServer = true.obs;

  Widget buildStatus() {
    final fontSize = 14.0;
    final textStyle = TextStyle(fontSize: fontSize);
    final light = Container(
      height: 8,
      width: 8,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(20),
        color: svcStopped.value || svcStatusCode.value == 0
            ? kColorWarn
            : (svcStatusCode.value == 1
                ? Color.fromARGB(255, 50, 190, 166)
                : Color.fromARGB(255, 224, 79, 95)),
      ),
    ).paddingSymmetric(horizontal: 12.0);
    if (svcStopped.value) {
      return Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          light,
          Text(translate("Service is not running"), style: textStyle),
          TextButton(
              onPressed: () async {
                bool checked = await bind.mainCheckSuperUserPermission();
                if (checked) {
                  bind.mainSetOption(key: "stop-service", value: "");
                }
              },
              child: Text(translate("Start Service"), style: textStyle))
        ],
      );
    } else {
      if (svcStatusCode.value == 0) {
        return Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            light,
            Text(translate("connecting_status"), style: textStyle)
          ],
        );
      } else if (svcStatusCode.value == -1) {
        return Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            light,
            Text(translate("not_ready_status"), style: textStyle)
          ],
        );
      }
    }
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        light,
        Text(translate('Ready'), style: textStyle),
        Text(', ', style: textStyle),
        svcIsUsingPublicServer.value
            ? InkWell(
                onTap: onUsePublicServerGuide,
                child: Text(
                  translate('setup_server_tip'),
                  style: TextStyle(
                      decoration: TextDecoration.underline, fontSize: fontSize),
                ),
              )
            : Offstage()
      ],
    );
  }

  void onUsePublicServerGuide() {
    const url = "https://rustdesk.com/blog/id-relay-set/";
    canLaunchUrlString(url).then((can) {
      if (can) {
        launchUrlString(url);
      }
    });
  }

  updateStatus() async {
    svcStopped.value = await bind.mainGetOption(key: "stop-service") == "Y";
    final status =
        jsonDecode(await bind.mainGetConnectStatus()) as Map<String, dynamic>;
    svcStatusCode.value = status["status_num"];
    svcIsUsingPublicServer.value = await bind.mainIsUsingPublicServer();
  }
}
