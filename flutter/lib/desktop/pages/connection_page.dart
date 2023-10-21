// main window right pane

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:auto_size_text/auto_size_text.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher_string.dart';
import 'package:window_manager/window_manager.dart';
import 'package:flutter_hbb/models/peer_model.dart';

import '../../common.dart';
import '../../common/formatter/id_formatter.dart';
import '../../common/widgets/peer_tab_page.dart';
import '../../common/widgets/peer_card.dart';
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

  Timer? _updateTimer;

  final RxBool _idInputFocused = false.obs;

  var svcStopped = Get.find<RxBool>(tag: 'stop-service');
  var svcIsUsingPublicServer = true.obs;

  bool isWindowMinimized = false;
  List<Peer> peers = [];
  List _frontN<T>(List list, int n) {
    if (list.length <= n) {
      return list;
    } else {
      return list.sublist(0, n);
    }
  }
  bool isPeersLoading = false;

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
    Get.put<IDTextEditingController>(_idController);
    windowManager.addListener(this);
  }

  @override
  void dispose() {
    _idController.dispose();
    _updateTimer?.cancel();
    windowManager.removeListener(this);
    if (Get.isRegistered<IDTextEditingController>()) {
      Get.delete<IDTextEditingController>();
    }
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
    stateGlobal.resizeEdgeSize.value =
        stateGlobal.isMaximized.isTrue ? kMaximizeEdgeSize : kWindowEdgeSize;
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
            child: Column(
          children: [
            Row(
              children: [
                Flexible(child: _buildRemoteIDTextField(context)),
              ],
            ).marginOnly(top: 22),
            SizedBox(height: 12),
            Divider().paddingOnly(right: 12),
            Expanded(child: PeerTabPage()),
          ],
        ).paddingOnly(left: 12.0)),
        const Divider(height: 1),
        buildStatus()
      ],
    );
  }

  /// Callback for the connect button.
  /// Connects to the selected peer.
  void onConnect({bool isFileTransfer = false}) {
    var id = _idController.id;
    connect(context, id, isFileTransfer: isFileTransfer);
  }

  Future<void> _fetchPeers() async {
    setState(() {
      isPeersLoading = true;
    });
    await Future.delayed(Duration(milliseconds: 100));
    await _getAllPeers();
    setState(() {
        isPeersLoading = false;
      });
  }

  Future<void> _getAllPeers() async {
    Map<String, dynamic> recentPeers = jsonDecode(await bind.mainLoadRecentPeersSync());
    Map<String, dynamic> lanPeers = jsonDecode(await bind.mainLoadLanPeersSync());
    Map<String, dynamic> abPeers = jsonDecode(await bind.mainLoadAbSync());
    Map<String, dynamic> groupPeers = jsonDecode(await bind.mainLoadGroupSync());

    Map<String, dynamic> combinedPeers = {};

    void mergePeers(Map<String, dynamic> peers) {
      if (peers.containsKey("peers")) {
        dynamic peerData = peers["peers"];

        if (peerData is String) {
          try {
            peerData = jsonDecode(peerData);
          } catch (e) {
            debugPrint("Error decoding peers: $e");
            return;
          }
        }

        if (peerData is List) {
          for (var peer in peerData) {
            if (peer is Map && peer.containsKey("id")) {
              String id = peer["id"];
              if (id != null && !combinedPeers.containsKey(id)) {
                combinedPeers[id] = peer;
              }
            }
          }
        }
      }
    }

    mergePeers(recentPeers);
    mergePeers(lanPeers);
    mergePeers(abPeers);
    mergePeers(groupPeers);

      List<Peer> parsedPeers = [];

    for (var peer in combinedPeers.values) {
      parsedPeers.add(Peer.fromJson(peer));
    }
      peers = parsedPeers;
  }

  /// UI for the remote ID TextField.
  /// Search for a peer.
  Widget _buildRemoteIDTextField(BuildContext context) {
    var w = Container(
      width: 320 + 20 * 2,
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 22),
      decoration: BoxDecoration(
          borderRadius: const BorderRadius.all(Radius.circular(13)),
          border: Border.all(color: Theme.of(context).colorScheme.background)),
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
                  child: 
                   Autocomplete<Peer>(
                    optionsBuilder: (TextEditingValue textEditingValue) {
                      if (textEditingValue.text == '') {
                        return const Iterable<Peer>.empty();
                      }
                      else if (peers.isEmpty) {
                         Peer emptyPeer = Peer(
                          id: '',
                          username: '',
                          hostname: '',
                          alias: '',
                          platform: '',
                          tags: [],
                          hash: '',
                          forceAlwaysRelay: false,
                          rdpPort: '',
                          rdpUsername: '',
                          loginName: '',
                        );
                        return [emptyPeer];
                      }
                      else {
                        String textWithoutSpaces = textEditingValue.text.replaceAll(" ", "");
                        if (int.tryParse(textWithoutSpaces) != null) {
                          textEditingValue = TextEditingValue(
                            text: textWithoutSpaces,
                            selection: textEditingValue.selection,
                          );
                        }
                        String textToFind = textEditingValue.text.toLowerCase();

                        return peers.where((peer) =>
                        peer.id.toLowerCase().contains(textToFind) ||
                        peer.username.toLowerCase().contains(textToFind) ||
                        peer.hostname.toLowerCase().contains(textToFind) ||
                        peer.alias.toLowerCase().contains(textToFind))
                            .toList();
                      }
                    },

                    fieldViewBuilder: (BuildContext context,
                        TextEditingController fieldTextEditingController,
                        FocusNode fieldFocusNode ,
                        VoidCallback onFieldSubmitted,
                        ) {
                      fieldTextEditingController.text = _idController.text;
                      fieldFocusNode.addListener(() async {
                        _idInputFocused.value = fieldFocusNode.hasFocus;
                        if (fieldFocusNode.hasFocus && !isPeersLoading){
                          _fetchPeers();
                        }
                      });
                      final textLength = fieldTextEditingController.value.text.length;
                      // select all to facilitate removing text, just following the behavior of address input of chrome
                      fieldTextEditingController.selection = TextSelection(baseOffset: 0, extentOffset: textLength);
                      return Obx(() =>
                      TextField(
                        maxLength: 90,
                        autocorrect: false,
                        enableSuggestions: false,
                        keyboardType: TextInputType.visiblePassword,
                        focusNode: fieldFocusNode,
                        style: const TextStyle(
                          fontFamily: 'WorkSans',
                          fontSize: 22,
                          height: 1.4,
                        ),
                        maxLines: 1,
                        cursorColor: Theme.of(context).textTheme.titleLarge?.color,
                        decoration: InputDecoration(
                            filled: false,
                            counterText: '',
                            hintText: _idInputFocused.value
                                ? null
                                : translate('Enter Remote ID'),
                            contentPadding: const EdgeInsets.symmetric(
                                horizontal: 15, vertical: 13)),
                        controller: fieldTextEditingController,
                        inputFormatters: [IDTextInputFormatter()],
                        onChanged: (v) {
                          _idController.id = v;
                        },
                        onSubmitted: (s) {
                          if (s == '') {
                            return;
                          }
                          try {
                            final id = int.parse(s);
                            _idController.id = s;
                            onConnect();
                          } catch (_) {
                            return;
                          }
                        },
                      ));
                    },
                    optionsViewBuilder: (BuildContext context, AutocompleteOnSelected<Peer> onSelected, Iterable<Peer> options) {
                      double maxHeight = 0;
                      for (var peer in options) {
                        if (maxHeight < 200) {
                          maxHeight += 50;
                        }
                      }
                      return Align(
                        alignment: Alignment.topLeft,
                        child: ClipRRect(
                          borderRadius: BorderRadius.circular(5),
                          child: Material(
                          elevation: 4,
                          child: ConstrainedBox(
                            constraints: BoxConstraints(
                              maxHeight: maxHeight,
                              maxWidth: 319,
                            ),
                              child: peers.isEmpty && isPeersLoading
                              ? Container(
                                    height: 80,
                                     child: Center( 
                                      child: CircularProgressIndicator(
                                        strokeWidth: 2,
                                      ),
                                    )
                                  )
                              : Padding(
                              padding: const EdgeInsets.only(top: 5),
                              child: ListView(
                                children: options
                                    .map((peer) => _buildPeerTile(context, peer))
                                    .toList()
                              ),
                            ),
                          ),
                        )),
                      );
                    },
                  )
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
                    onTap: () => onConnect(isFileTransfer: true),
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

  Widget _buildPeerTile(
      BuildContext context, Peer peer) {
        final double _tileRadius = 5;
        final name =
          '${peer.username}${peer.username.isNotEmpty && peer.hostname.isNotEmpty ? '@' : ''}${peer.hostname}';
        final greyStyle = TextStyle(
          fontSize: 11,
          color: Theme.of(context).textTheme.titleLarge?.color?.withOpacity(0.6));
        final child = GestureDetector(
          onTap: () {
            setState(() {
              _idController.id = peer.id;
              FocusScope.of(context).unfocus();
            });
          },
          child:
        Container(
          height: 42,
          margin: EdgeInsets.only(bottom: 5),
          child: Row(
          mainAxisSize: MainAxisSize.max,
          children: [
            Container(
              decoration: BoxDecoration(
                color: str2color('${peer.id}${peer.platform}', 0x7f),
                borderRadius: BorderRadius.only(
                        topLeft: Radius.circular(_tileRadius),
                        bottomLeft: Radius.circular(_tileRadius),
                      ),
              ),
              alignment: Alignment.center,
              width: 42,
              height: null,
              child: getPlatformImage(peer.platform, size: 30)
                  .paddingAll(6),
            ),
            Expanded(
              child: Container(
                decoration: BoxDecoration(
                  color: Theme.of(context).colorScheme.background,
                  borderRadius: BorderRadius.only(
                    topRight: Radius.circular(_tileRadius),
                    bottomRight: Radius.circular(_tileRadius),
                  ),
                ),
            child: Row(
              children: [
                Expanded(
                  child: Column(
                    children: [
                      Row(children: [
                        getOnline(8, peer.online),
                        Expanded(
                            child: Text(
                          peer.alias.isEmpty ? formatID(peer.id) : peer.alias,
                          overflow: TextOverflow.ellipsis,
                          style: Theme.of(context).textTheme.titleSmall,
                        )),
                        !peer.alias.isEmpty?
                        Padding(
                          padding: const EdgeInsets.only(left: 5, right: 5),
                          child: Text(
                            "(${peer.id})",
                            style: greyStyle,
                            overflow: TextOverflow.ellipsis,
                          )
                        )
                        : Container(),
                      ]).marginOnly(top: 2),
                      Align(
                        alignment: Alignment.centerLeft,
                        child: Text(
                          name,
                          style: greyStyle,
                          textAlign: TextAlign.start,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ).marginOnly(top: 2),
                ),
              ],
            ).paddingOnly(left: 10.0, top: 3.0),
            ),
        )
      ],
    )));
    final colors =
        _frontN(peer.tags, 25).map((e) => gFFI.abModel.getTagColor(e)).toList();
    return Tooltip(
      message: isMobile
          ? ''
          : peer.tags.isNotEmpty
              ? '${translate('Tags')}: ${peer.tags.join(', ')}'
              : '',
      child: Stack(children: [
        child,
        if (colors.isNotEmpty)
          Positioned(
            top: 5,
            right: 10,
            child: CustomPaint(
              painter: TagPainter(radius: 3, colors: colors),
            ),
          )
      ]),
    );
  }

  Widget buildStatus() {
    final em = 14.0;
    return Container(
      height: 3 * em,
      child: Obx(() => Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Container(
                height: 8,
                width: 8,
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(4),
                  color: svcStopped.value ||
                          stateGlobal.svcStatus.value == SvcStatus.connecting
                      ? kColorWarn
                      : (stateGlobal.svcStatus.value == SvcStatus.ready
                          ? Color.fromARGB(255, 50, 190, 166)
                          : Color.fromARGB(255, 224, 79, 95)),
                ),
              ).marginSymmetric(horizontal: em),
              Text(
                  svcStopped.value
                      ? translate("Service is not running")
                      : stateGlobal.svcStatus.value == SvcStatus.connecting
                          ? translate("connecting_status")
                          : stateGlobal.svcStatus.value == SvcStatus.notReady
                              ? translate("not_ready_status")
                              : translate('Ready'),
                  style: TextStyle(fontSize: em)),
              // stop
              Offstage(
                offstage: !svcStopped.value,
                child: InkWell(
                        onTap: () async {
                          await start_service(true);
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
                      stateGlobal.svcStatus.value == SvcStatus.ready &&
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
    final statusNum = status['status_num'] as int;
    final preStatus = stateGlobal.svcStatus.value;
    if (statusNum == 0) {
      stateGlobal.svcStatus.value = SvcStatus.connecting;
    } else if (statusNum == -1) {
      stateGlobal.svcStatus.value = SvcStatus.notReady;
    } else if (statusNum == 1) {
      stateGlobal.svcStatus.value = SvcStatus.ready;
      if (preStatus != SvcStatus.ready) {
        gFFI.userModel.refreshCurrentUser();
      }
    } else {
      stateGlobal.svcStatus.value = SvcStatus.notReady;
    }
    svcIsUsingPublicServer.value = await bind.mainIsUsingPublicServer();
  }
}
