// main window right pane

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:auto_size_text/auto_size_text.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/scroll_wrapper.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher_string.dart';
import 'package:window_manager/window_manager.dart';

import '../../common.dart';
import '../../common/formatter/id_formatter.dart';
import '../../common/widgets/peer_tab_page.dart';
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
    with SingleTickerProviderStateMixin, WindowListener {
  /// Controller for the id input bar.
  final _idController = IDTextEditingController();

  /// Nested scroll controller
  final _scrollController = ScrollController();

  Timer? _updateTimer;

  final RxBool _idInputFocused = false.obs;
  final FocusNode _idFocusNode = FocusNode();

  var svcStopped = Get.find<RxBool>(tag: 'stop-service');
  var svcStatusCode = 0.obs;
  var svcIsUsingPublicServer = true.obs;

  bool isWindowMinimized = false;

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
    _updateTimer = periodic_immediate(Duration(seconds: 1), () async {
      updateStatus();
    });
    _idFocusNode.addListener(() {
      _idInputFocused.value = _idFocusNode.hasFocus;
      // select all to faciliate removing text, just following the behavior of address input of chrome
      _idController.selection = TextSelection(
          baseOffset: 0, extentOffset: _idController.value.text.length);
    });
    windowManager.addListener(this);
  }

  @override
  void dispose() {
    _idController.dispose();
    _updateTimer?.cancel();
    windowManager.removeListener(this);
    super.dispose();
  }

  @override
  void onWindowEvent(String eventName) {
    super.onWindowEvent(eventName);
    if (eventName == 'minimize') {
      isWindowMinimized = true;
    } else if (eventName == 'maximize' || eventName == 'restore') {
      if (isWindowMinimized && Platform.isWindows) {
        // windows can't update when minimized.
        Get.forceAppUpdate();
      }
      isWindowMinimized = false;
    }
  }

  @override
  void onWindowEnterFullScreen() {
    // Remove edge border by setting the value to zero.
    stateGlobal.resizeEdgeSize.value = 0;
  }

  @override
  void onWindowLeaveFullScreen() {
    // Restore edge border to default edge size.
    stateGlobal.resizeEdgeSize.value = kWindowEdgeSize;
  }

  @override
  void onWindowClose() {
    super.onWindowClose();
    bind.mainOnMainWindowClose();
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
              physics: DraggableNeverScrollableScrollPhysics(),
              slivers: [
                SliverList(
                    delegate: SliverChildListDelegate([
                  Row(
                    children: [
                      Flexible(child: _buildRemoteIDTextField(context)),
                    ],
                  ).marginOnly(top: 22),
                  SizedBox(height: 12),
                  Divider().paddingOnly(right: 12),
                ])),
                SliverFillRemaining(
                  hasScrollBody: false,
                  child: PeerTabPage().paddingOnly(right: 12.0),
                )
              ],
            ).paddingOnly(left: 12.0),
          ),
        ),
        const Divider(height: 1),
        buildStatus()
      ],
    );
  }

  /// Callback for the connect button.
  /// Connects to the selected peer.
  void onConnect({bool isFileTransfer = false}) {
    var id = _idController.id;
    var forceRelay = id.endsWith(r'/r');
    if (forceRelay) id = id.substring(0, id.length - 2);
    connect(context, id,
        isFileTransfer: isFileTransfer, forceRelay: forceRelay);
  }

  /// UI for the remote ID TextField.
  /// Search for a peer and connect to it if the id exists.
  Widget _buildRemoteIDTextField(BuildContext context) {
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
                Expanded(
                  child: AutoSizeText(
                    translate('Control Remote Desktop'),
                    maxLines: 1,
                    style: Theme.of(context)
                        .textTheme
                        .titleLarge
                        ?.merge(TextStyle(height: 1)),
                  ),
                ),
              ],
            ).marginOnly(bottom: 15),
            Row(
              children: [
                Expanded(
                  child: Obx(
                    () => TextField(
                      maxLength: 90,
                      autocorrect: false,
                      enableSuggestions: false,
                      keyboardType: TextInputType.visiblePassword,
                      focusNode: _idFocusNode,
                      style: const TextStyle(
                        fontFamily: 'WorkSans',
                        fontSize: 22,
                        height: 1.25,
                      ),
                      maxLines: 1,
                      cursorColor:
                          Theme.of(context).textTheme.titleLarge?.color,
                      decoration: InputDecoration(
                          counterText: '',
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
    return Container(
        constraints: const BoxConstraints(maxWidth: 600), child: w);
  }

  Widget buildStatus() {
    final em = 14.0;
    return ConstrainedBox(
      constraints: BoxConstraints.tightFor(height: 3 * em),
      child: Obx(() => Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Container(
                height: 8,
                width: 8,
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(4),
                  color: svcStopped.value || svcStatusCode.value == 0
                      ? kColorWarn
                      : (svcStatusCode.value == 1
                          ? Color.fromARGB(255, 50, 190, 166)
                          : Color.fromARGB(255, 224, 79, 95)),
                ),
              ).marginSymmetric(horizontal: em),
              Text(
                  svcStopped.value
                      ? translate("Service is not running")
                      : svcStatusCode.value == 0
                          ? translate("connecting_status")
                          : svcStatusCode.value == -1
                              ? translate("not_ready_status")
                              : translate('Ready'),
                  style: TextStyle(fontSize: em)),
              // stop
              Offstage(
                offstage: !svcStopped.value,
                child: InkWell(
                        onTap: () async {
                          bool checked = !bind.mainIsInstalled() ||
                              await bind.mainCheckSuperUserPermission();
                          if (checked) {
                            bind.mainSetOption(key: "stop-service", value: "");
                            bind.mainSetOption(key: "access-mode", value: "");
                          }
                        },
                        child: Text(translate("Start Service"),
                            style: TextStyle(
                                decoration: TextDecoration.underline,
                                fontSize: em)))
                    .marginOnly(left: em),
              ),
              // ready && public
              Flexible(
                child: Offstage(
                  offstage: !(!svcStopped.value &&
                      svcStatusCode.value == 1 &&
                      svcIsUsingPublicServer.value),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: [
                      Text(', ', style: TextStyle(fontSize: em)),
                      Flexible(
                        child: InkWell(
                          onTap: onUsePublicServerGuide,
                          child: Row(
                            children: [
                              Flexible(
                                child: Text(
                                  translate('setup_server_tip'),
                                  style: TextStyle(
                                      decoration: TextDecoration.underline,
                                      fontSize: em),
                                ),
                              ),
                            ],
                          ),
                        ),
                      )
                    ],
                  ),
                ),
              )
            ],
          )),
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
    final status =
        jsonDecode(await bind.mainGetConnectStatus()) as Map<String, dynamic>;
    svcStatusCode.value = status["status_num"];
    svcIsUsingPublicServer.value = await bind.mainIsUsingPublicServer();
  }
}
