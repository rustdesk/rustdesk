import 'dart:async';
import 'dart:convert';

import 'package:auto_size_text_field/auto_size_text_field.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:flutter_hbb/models/peer_model.dart';

import '../../common.dart';
import '../../common/widgets/login.dart';
import '../../common/widgets/peer_tab_page.dart';
import '../../common/widgets/peer_card.dart';
import '../../consts.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import 'home_page.dart';
import 'scan_page.dart';
import 'settings_page.dart';

/// Connection page for connecting to a remote peer.
class ConnectionPage extends StatefulWidget implements PageShape {
  ConnectionPage({Key? key}) : super(key: key);

  @override
  final icon = const Icon(Icons.connected_tv);

  @override
  final title = translate("Connection");

  @override
  final appBarActions = isWeb ? <Widget>[const WebMenu()] : <Widget>[];

  @override
  State<ConnectionPage> createState() => _ConnectionPageState();
}

/// State for the connection page.
class _ConnectionPageState extends State<ConnectionPage> {
  /// Controller for the id input bar.
  final _idController = IDTextEditingController();
  final RxBool _idEmpty = true.obs;

  /// Update url. If it's not null, means an update is available.
  var _updateUrl = '';
  List<Peer> peers = [];
  List _frontN<T>(List list, int n) {
    if (list.length <= n) {
      return list;
    } else {
      return list.sublist(0, n);
    }
  }
  bool isPeersLoading = false;
  bool isPeersLoaded = false;

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
    if (isAndroid) {
      Timer(const Duration(seconds: 1), () async {
        _updateUrl = await bind.mainGetSoftwareUpdateUrl();
        if (_updateUrl.isNotEmpty) setState(() {});
      });
    }

    _idController.addListener(() {
      _idEmpty.value = _idController.text.isEmpty;
    });
    Get.put<IDTextEditingController>(_idController);
  }

  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    return CustomScrollView(
      slivers: [
        SliverList(
            delegate: SliverChildListDelegate([
          _buildUpdateUI(),
          _buildRemoteIDTextField(),
        ])),
        SliverFillRemaining(
          hasScrollBody: true,
          child: PeerTabPage(),
        )
      ],
    ).marginOnly(top: 2, left: 10, right: 10);
  }

  /// Callback for the connect button.
  /// Connects to the selected peer.
  void onConnect() {
    var id = _idController.id;
    connect(context, id);
  }

  /// UI for software update.
  /// If [_updateUrl] is not empty, shows a button to update the software.
  Widget _buildUpdateUI() {
    return _updateUrl.isEmpty
        ? const SizedBox(height: 0)
        : InkWell(
            onTap: () async {
              final url = 'https://rustdesk.com/download';
              if (await canLaunchUrl(Uri.parse(url))) {
                await launchUrl(Uri.parse(url));
              }
            },
            child: Container(
                alignment: AlignmentDirectional.center,
                width: double.infinity,
                color: Colors.pinkAccent,
                padding: const EdgeInsets.symmetric(vertical: 12),
                child: Text(translate('Download new version'),
                    style: const TextStyle(
                        color: Colors.white, fontWeight: FontWeight.bold))));
  }

  Future<void> _fetchPeers() async {
    setState(() {
      isPeersLoading = true;
    });
    await Future.delayed(Duration(milliseconds: 100));
    await _getAllPeers();
    setState(() {
        isPeersLoading = false;
        isPeersLoaded = true;
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
  /// Search for a peer and connect to it if the id exists.
  Widget _buildRemoteIDTextField() {
    final w = SizedBox(
      height: 84,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 2),
        child: Ink(
          decoration: BoxDecoration(
            color: Theme.of(context).cardColor,
            borderRadius: BorderRadius.all(Radius.circular(13)),
          ),
          child: Row(
            children: <Widget>[
              Expanded(
                child: Container(
                  padding: const EdgeInsets.only(left: 16, right: 16),
                  child: Autocomplete<Peer>(
                    optionsBuilder: (TextEditingValue textEditingValue) {
                      if (textEditingValue.text == '') {
                        return const Iterable<Peer>.empty();
                      }
                      else if (peers.isEmpty && !isPeersLoaded) {
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
                      FocusNode fieldFocusNode, VoidCallback onFieldSubmitted) {
                      fieldTextEditingController.text = _idController.text;
                      fieldFocusNode.addListener(() async{
                      _idEmpty.value = fieldTextEditingController.text.isEmpty;
                        if (fieldFocusNode.hasFocus && !isPeersLoading){
                          _fetchPeers();
                        }
                      });
                      final textLength = fieldTextEditingController.value.text.length;
                      // select all to facilitate removing text, just following the behavior of address input of chrome
                      fieldTextEditingController.selection = TextSelection(baseOffset: 0, extentOffset: textLength);
                    return AutoSizeTextField(
                    controller: fieldTextEditingController,
                    focusNode: fieldFocusNode,
                    minFontSize: 18,
                    autocorrect: false,
                    enableSuggestions: false,
                    keyboardType: TextInputType.visiblePassword,
                    // keyboardType: TextInputType.number,
                    style: const TextStyle(
                      fontFamily: 'WorkSans',
                      fontWeight: FontWeight.bold,
                      fontSize: 30,
                      color: MyTheme.idColor,
                    ),
                    decoration: InputDecoration(
                      labelText: translate('Remote ID'),
                      // hintText: 'Enter your remote ID',
                      border: InputBorder.none,
                      helperStyle: const TextStyle(
                        fontWeight: FontWeight.bold,
                        fontSize: 16,
                        color: MyTheme.darkGray,
                      ),
                      labelStyle: const TextStyle(
                        fontWeight: FontWeight.w600,
                        fontSize: 16,
                        letterSpacing: 0.2,
                        color: MyTheme.darkGray,
                      ),
                    ),
                    inputFormatters: [IDTextInputFormatter()],
                     );
                    },
                    optionsViewBuilder: (BuildContext context, AutocompleteOnSelected<Peer> onSelected, Iterable<Peer> options) {
                      double maxHeight = options.length * 50;
                      maxHeight = maxHeight > 200 ? 200 : maxHeight;
                      return Align(
                        alignment: Alignment.topLeft,
                        child: ClipRRect(
                          borderRadius: BorderRadius.circular(5),
                          child: Material(
                          elevation: 4,
                          child: ConstrainedBox(
                            constraints: BoxConstraints(
                              maxHeight: maxHeight,
                              maxWidth: 320,
                            ),
                              child: peers.isEmpty && isPeersLoading
                              ? Container(
                                    height: 80,
                                     child: Center( 
                                      child: CircularProgressIndicator(
                                        strokeWidth: 2,
                                      )))
                              : ListView(
                              padding: EdgeInsets.only(top: 5),
                              children: options.map((peer) => _buildPeerTile(context, peer)).toList(),
                            ))))
                      );
                    },
                  ),
                ),
              ),
              Obx(() => Offstage(
                    offstage: _idEmpty.value,
                    child: IconButton(
                        onPressed: () {
                          setState(() {
                            _idController.clear();
                          });
                        },
                        icon: Icon(Icons.clear, color: MyTheme.darkGray)),
                  )),
              SizedBox(
                width: 60,
                height: 60,
                child: IconButton(
                  icon: const Icon(Icons.arrow_forward,
                      color: MyTheme.darkGray, size: 45),
                  onPressed: onConnect,
                ),
              ),
            ],
          ),
        ),
      ),
    );
    return Align(
        alignment: Alignment.topCenter,
        child: Container(constraints: kMobilePageConstraints, child: w));
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

  @override
  void dispose() {
    _idController.dispose();
    if (Get.isRegistered<IDTextEditingController>()) {
      Get.delete<IDTextEditingController>();
    }
    super.dispose();
  }
}

class WebMenu extends StatefulWidget {
  const WebMenu({Key? key}) : super(key: key);

  @override
  State<WebMenu> createState() => _WebMenuState();
}

class _WebMenuState extends State<WebMenu> {
  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    return PopupMenuButton<String>(
        tooltip: "",
        icon: const Icon(Icons.more_vert),
        itemBuilder: (context) {
          return (isIOS
                  ? [
                      const PopupMenuItem(
                        value: "scan",
                        child: Icon(Icons.qr_code_scanner, color: Colors.black),
                      )
                    ]
                  : <PopupMenuItem<String>>[]) +
              [
                PopupMenuItem(
                  value: "server",
                  child: Text(translate('ID/Relay Server')),
                )
              ] +
              [
                PopupMenuItem(
                  value: "login",
                  child: Text(gFFI.userModel.userName.value.isEmpty
                      ? translate("Login")
                      : '${translate("Logout")} (${gFFI.userModel.userName.value})'),
                )
              ] +
              [
                PopupMenuItem(
                  value: "about",
                  child: Text('${translate('About')} RustDesk'),
                )
              ];
        },
        onSelected: (value) {
          if (value == 'server') {
            showServerSettings(gFFI.dialogManager);
          }
          if (value == 'about') {
            showAbout(gFFI.dialogManager);
          }
          if (value == 'login') {
            if (gFFI.userModel.userName.value.isEmpty) {
              loginDialog();
            } else {
              logOutConfirmDialog();
            }
          }
          if (value == 'scan') {
            Navigator.push(
              context,
              MaterialPageRoute(
                builder: (BuildContext context) => ScanPage(),
              ),
            );
          }
        });
  }
}
