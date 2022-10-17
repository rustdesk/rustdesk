// main window right pane

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/address_book.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/scroll_wrapper.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher_string.dart';

import '../../common.dart';
import '../../common/formatter/id_formatter.dart';
import '../../common/widgets/peer_tab_page.dart';
import '../../common/widgets/peers_view.dart';
import '../../models/platform_model.dart';
import '../widgets/button.dart';

/// Connection page for connecting to a remote peer.
class ConnectionPage extends StatefulWidget {
  const ConnectionPage({Key? key}) : super(key: key);

  @override
  State<ConnectionPage> createState() => _ConnectionPageState();
}

/// State for the connection page.
class _ConnectionPageState extends State<ConnectionPage>
    with SingleTickerProviderStateMixin {
  /// Controller for the id input bar.
  final _idController = IDTextEditingController();

  /// Nested scroll controller
  final _scrollController = ScrollController();

  Timer? _updateTimer;

  final RxBool _idInputFocused = false.obs;
  final FocusNode _idFocusNode = FocusNode();

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
    return Column(
      children: [
        Expanded(
          child: DesktopScrollWrapper(
            scrollController: _scrollController,
            child: CustomScrollView(
              controller: _scrollController,
              physics: NeverScrollableScrollPhysics(),
              slivers: [
                SliverList(
                    delegate: SliverChildListDelegate([
                  Row(
                    children: [
                      _buildRemoteIDTextField(context),
                    ],
                  ).marginOnly(top: 22),
                  SizedBox(height: 12),
                  Divider().paddingOnly(right: 12),
                ])),
                SliverFillRemaining(
                  hasScrollBody: false,
                  child: PeerTabPage(
                    tabs: [
                      translate('Recent Sessions'),
                      translate('Favorites'),
                      translate('Discovered'),
                      translate('Address Book')
                    ],
                    children: [
                      RecentPeersView(
                        menuPadding: kDesktopMenuPadding,
                      ),
                      FavoritePeersView(
                        menuPadding: kDesktopMenuPadding,
                      ),
                      DiscoveredPeersView(
                        menuPadding: kDesktopMenuPadding,
                      ),
                      const AddressBook(
                        menuPadding: kDesktopMenuPadding,
                      ),
                    ],
                  ).paddingOnly(right: 12.0),
                )
              ],
            ).paddingOnly(left: 12.0),
          ),
        ),
        const Divider(),
        SizedBox(child: Obx(() => buildStatus()))
            .paddingOnly(bottom: 12, top: 6),
      ],
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
    _idFocusNode.addListener(() {
      _idInputFocused.value = _idFocusNode.hasFocus;
    });
    var w = Container(
      width: 320 + 20 * 2,
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 22),
      decoration: BoxDecoration(
        color: Theme.of(context).backgroundColor,
        borderRadius: const BorderRadius.all(Radius.circular(13)),
      ),
      child: Ink(
        child: Column(
          children: [
            Row(
              children: [
                Text(
                  translate('Control Remote Desktop'),
                  style: Theme.of(context)
                      .textTheme
                      .titleLarge
                      ?.merge(TextStyle(height: 1)),
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
                      focusNode: _idFocusNode,
                      style: const TextStyle(
                        fontFamily: 'WorkSans',
                        fontSize: 22,
                        height: 1,
                      ),
                      maxLines: 1,
                      cursorColor:
                          Theme.of(context).textTheme.titleLarge?.color,
                      decoration: InputDecoration(
                          hintText: _idInputFocused.value
                              ? null
                              : translate('Enter Remote ID'),
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
                  Button(
                    isOutline: true,
                    onTap: () {
                      onConnect(isFileTransfer: true);
                    },
                    text: "Transfer File",
                  ),
                  const SizedBox(
                    width: 17,
                  ),
                  Button(onTap: onConnect, text: "Connect"),
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
                  bind.mainSetOption(key: "access-mode", value: "");
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
        Offstage(
            offstage: !svcIsUsingPublicServer.value,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                Text(', ', style: textStyle),
                InkWell(
                  onTap: onUsePublicServerGuide,
                  child: Text(
                    translate('setup_server_tip'),
                    style: TextStyle(
                        decoration: TextDecoration.underline,
                        fontSize: fontSize),
                  ),
                )
              ],
            ))
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
