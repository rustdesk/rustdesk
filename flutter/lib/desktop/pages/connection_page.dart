import 'dart:async';
import 'dart:convert';

import 'package:contextmenu/contextmenu.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/widgets/peer_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher_string.dart';

import '../../common.dart';
import '../../mobile/pages/scan_page.dart';
import '../../mobile/pages/settings_page.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';

// enum RemoteType { recently, favorite, discovered, addressBook }

/// Connection page for connecting to a remote peer.
class ConnectionPage extends StatefulWidget {
  ConnectionPage({Key? key}) : super(key: key);

  @override
  _ConnectionPageState createState() => _ConnectionPageState();
}

/// State for the connection page.
class _ConnectionPageState extends State<ConnectionPage> {
  /// Controller for the id input bar.
  final _idController = TextEditingController();

  /// Update url. If it's not null, means an update is available.
  var _updateUrl = '';

  Timer? _updateTimer;

  @override
  void initState() {
    super.initState();
    if (_idController.text.isEmpty) {
      () async {
        final lastRemoteId = await bind.mainGetLastRemoteId();
        if (lastRemoteId != _idController.text) {
          setState(() {
            _idController.text = lastRemoteId;
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
            getUpdateUI(),
            Row(
              children: [
                getSearchBarUI(context),
              ],
            ).marginOnly(top: 22, left: 22),
            SizedBox(height: 12),
            Divider(
              thickness: 1,
              indent: 22,
              endIndent: 22,
            ),
            Expanded(
                // TODO: move all tab info into _PeerTabbedPage
                child: _PeerTabbedPage(
              tabs: [
                translate('Recent Sessions'),
                translate('Favorites'),
                translate('Discovered'),
                translate('Address Book')
              ],
              children: [
                RecentPeerWidget(),
                FavoritePeerWidget(),
                DiscoveredPeerWidget(),
                // AddressBookPeerWidget(),
                // FutureBuilder<Widget>(
                //     future: getPeers(rType: RemoteType.recently),
                //     builder: (context, snapshot) {
                //       if (snapshot.hasData) {
                //         return snapshot.data!;
                //       } else {
                //         return Offstage();
                //       }
                //     }),
                // FutureBuilder<Widget>(
                //     future: getPeers(rType: RemoteType.favorite),
                //     builder: (context, snapshot) {
                //       if (snapshot.hasData) {
                //         return snapshot.data!;
                //       } else {
                //         return Offstage();
                //       }
                //     }),
                // FutureBuilder<Widget>(
                //     future: getPeers(rType: RemoteType.discovered),
                //     builder: (context, snapshot) {
                //       if (snapshot.hasData) {
                //         return snapshot.data!;
                //       } else {
                //         return Offstage();
                //       }
                //     }),
                FutureBuilder<Widget>(
                    future: buildAddressBook(context),
                    builder: (context, snapshot) {
                      if (snapshot.hasData) {
                        return snapshot.data!;
                      } else {
                        return Offstage();
                      }
                    }),
              ],
            ).marginSymmetric(horizontal: 6)),
            Divider(),
            SizedBox(height: 50, child: Obx(() => buildStatus()))
                .paddingSymmetric(horizontal: 12.0)
          ]),
    );
  }

  /// Callback for the connect button.
  /// Connects to the selected peer.
  void onConnect({bool isFileTransfer = false}) {
    var id = _idController.text.trim();
    connect(id, isFileTransfer: isFileTransfer);
  }

  /// Connect to a peer with [id].
  /// If [isFileTransfer], starts a session only for file transfer.
  void connect(String id, {bool isFileTransfer = false}) async {
    if (id == '') return;
    id = id.replaceAll(' ', '');
    if (isFileTransfer) {
      await rustDeskWinManager.new_file_transfer(id);
    } else {
      await rustDeskWinManager.new_remote_desktop(id);
    }
    FocusScopeNode currentFocus = FocusScope.of(context);
    if (!currentFocus.hasPrimaryFocus) {
      currentFocus.unfocus();
    }
  }

  /// UI for software update.
  /// If [_updateUrl] is not empty, shows a button to update the software.
  Widget getUpdateUI() {
    return _updateUrl.isEmpty
        ? SizedBox(height: 0)
        : InkWell(
            onTap: () async {
              final url = _updateUrl + '.apk';
              if (await canLaunchUrlString(url)) {
                await launchUrlString(url);
              }
            },
            child: Container(
                alignment: AlignmentDirectional.center,
                width: double.infinity,
                color: Colors.pinkAccent,
                padding: EdgeInsets.symmetric(vertical: 12),
                child: Text(translate('Download new version'),
                    style: TextStyle(
                        color: Colors.white, fontWeight: FontWeight.bold))));
  }

  /// UI for the search bar.
  /// Search for a peer and connect to it if the id exists.
  Widget getSearchBarUI(BuildContext context) {
    RxBool ftHover = false.obs;
    RxBool ftPressed = false.obs;
    RxBool connHover = false.obs;
    RxBool connPressed = false.obs;
    var w = Container(
      width: 320 + 20 * 2,
      padding: EdgeInsets.only(left: 20, right: 20, bottom: 22, top: 30),
      decoration: BoxDecoration(
        color: MyTheme.color(context).bg,
        borderRadius: const BorderRadius.all(Radius.circular(13)),
      ),
      child: Ink(
        child: Column(
          children: [
            Row(
              children: <Widget>[
                Expanded(
                  child: Container(
                    child: TextField(
                      autocorrect: false,
                      enableSuggestions: false,
                      keyboardType: TextInputType.visiblePassword,
                      style: TextStyle(
                        fontFamily: 'WorkSans',
                        fontSize: 22,
                      ),
                      decoration: InputDecoration(
                        labelText: translate('Control Remote Desktop'),
                        border:
                            OutlineInputBorder(borderRadius: BorderRadius.zero),
                        helperStyle: TextStyle(
                          fontWeight: FontWeight.bold,
                          fontSize: 16,
                        ),
                        labelStyle: TextStyle(
                          fontWeight: FontWeight.w500,
                          fontSize: 26,
                          letterSpacing: 0.2,
                        ),
                      ),
                      controller: _idController,
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
                          height: 24,
                          width: 72,
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
                          ),
                        ),
                      )),
                  SizedBox(
                    width: 17,
                  ),
                  Obx(
                    () => InkWell(
                      onTapDown: (_) => connPressed.value = true,
                      onTapUp: (_) => connPressed.value = false,
                      onTapCancel: () => connPressed.value = false,
                      onHover: (value) => connHover.value = value,
                      onTap: onConnect,
                      child: Container(
                        height: 24,
                        width: 65,
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
                              "Connection",
                            ),
                            style: TextStyle(
                                fontSize: 12, color: MyTheme.color(context).bg),
                          ),
                        ),
                      ),
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
        child: Container(constraints: BoxConstraints(maxWidth: 600), child: w));
  }

  @override
  void dispose() {
    _idController.dispose();
    _updateTimer?.cancel();
    super.dispose();
  }

  /// Get the image for the current [platform].
  Widget getPlatformImage(String platform) {
    platform = platform.toLowerCase();
    if (platform == 'mac os')
      platform = 'mac';
    else if (platform != 'linux' && platform != 'android') platform = 'win';
    return Image.asset('assets/$platform.png', height: 50);
  }

  bool hitTag(List<dynamic> selectedTags, List<dynamic> idents) {
    if (selectedTags.isEmpty) {
      return true;
    }
    if (idents.isEmpty) {
      return false;
    }
    for (final tag in selectedTags) {
      if (!idents.contains(tag)) {
        return false;
      }
    }
    return true;
  }

  // /// Show the peer menu and handle user's choice.
  // /// User might remove the peer or send a file to the peer.
  // void showPeerMenu(BuildContext context, String id, RemoteType rType) async {
  //   var items = [
  //     PopupMenuItem<String>(
  //         child: Text(translate('Connect')), value: 'connect'),
  //     PopupMenuItem<String>(
  //         child: Text(translate('Transfer File')), value: 'file'),
  //     PopupMenuItem<String>(
  //         child: Text(translate('TCP Tunneling')), value: 'tcp-tunnel'),
  //     PopupMenuItem<String>(child: Text(translate('Rename')), value: 'rename'),
  //     rType == RemoteType.addressBook
  //         ? PopupMenuItem<String>(
  //             child: Text(translate('Remove')), value: 'ab-delete')
  //         : PopupMenuItem<String>(
  //             child: Text(translate('Remove')), value: 'remove'),
  //     PopupMenuItem<String>(
  //         child: Text(translate('Unremember Password')),
  //         value: 'unremember-password'),
  //   ];
  //   if (rType == RemoteType.favorite) {
  //     items.add(PopupMenuItem<String>(
  //         child: Text(translate('Remove from Favorites')),
  //         value: 'remove-fav'));
  //   } else if (rType != RemoteType.addressBook) {
  //     items.add(PopupMenuItem<String>(
  //         child: Text(translate('Add to Favorites')), value: 'add-fav'));
  //   } else {
  //     items.add(PopupMenuItem<String>(
  //         child: Text(translate('Edit Tag')), value: 'ab-edit-tag'));
  //   }
  //   var value = await showMenu(
  //     context: context,
  //     position: this._menuPos,
  //     items: items,
  //     elevation: 8,
  //   );
  //   if (value == 'remove') {
  //     setState(() => gFFI.setByName('remove', '$id'));
  //     () async {
  //       removePreference(id);
  //     }();
  //   } else if (value == 'file') {
  //     connect(id, isFileTransfer: true);
  //   } else if (value == 'add-fav') {
  //   } else if (value == 'connect') {
  //     connect(id, isFileTransfer: false);
  //   } else if (value == 'ab-delete') {
  //     gFFI.abModel.deletePeer(id);
  //     await gFFI.abModel.updateAb();
  //     setState(() {});
  //   } else if (value == 'ab-edit-tag') {
  //     abEditTag(id);
  //   }
  // }

  var svcStopped = false.obs;
  var svcStatusCode = 0.obs;
  var svcIsUsingPublicServer = true.obs;

  Widget buildStatus() {
    final light = Container(
      height: 8,
      width: 8,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(20),
        color: svcStopped.value ? Colors.redAccent : Colors.green,
      ),
    ).paddingSymmetric(horizontal: 10.0);
    if (svcStopped.value) {
      return Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          light,
          Text(translate("Service is not running")),
          TextButton(
              onPressed: () =>
                  bind.mainSetOption(key: "stop-service", value: ""),
              child: Text(translate("Start Service")))
        ],
      );
    } else {
      if (svcStatusCode.value == 0) {
        return Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [light, Text(translate("connecting_status"))],
        );
      } else if (svcStatusCode.value == -1) {
        return Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [light, Text(translate("not_ready_status"))],
        );
      }
    }
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        light,
        Text("${translate('Ready')}"),
        svcIsUsingPublicServer.value
            ? InkWell(
                onTap: onUsePublicServerGuide,
                child: Text(
                  ', ${translate('setup_server_tip')}',
                  style: TextStyle(decoration: TextDecoration.underline),
                ),
              )
            : Offstage()
      ],
    );
  }

  void onUsePublicServerGuide() {
    final url = "https://rustdesk.com/blog/id-relay-set/";
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

  handleLogin() {
    loginDialog().then((success) {
      if (success) {
        setState(() {});
      }
    });
  }

  Future<Widget> buildAddressBook(BuildContext context) async {
    final token = await bind.mainGetLocalOption(key: 'access_token');
    if (token.trim().isEmpty) {
      return Center(
        child: InkWell(
          onTap: handleLogin,
          child: Text(
            translate("Login"),
            style: TextStyle(decoration: TextDecoration.underline),
          ),
        ),
      );
    }
    final model = gFFI.abModel;
    return FutureBuilder(
        future: model.getAb(),
        builder: (context, snapshot) {
          if (snapshot.hasData) {
            return _buildAddressBook(context);
          } else if (snapshot.hasError) {
            return Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Text(translate("${snapshot.error}")),
                TextButton(
                    onPressed: () {
                      setState(() {});
                    },
                    child: Text(translate("Retry")))
              ],
            );
          } else {
            if (model.abLoading) {
              return Center(
                child: CircularProgressIndicator(),
              );
            } else if (model.abError.isNotEmpty) {
              return Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Text(translate("${model.abError}")),
                    TextButton(
                        onPressed: () {
                          setState(() {});
                        },
                        child: Text(translate("Retry")))
                  ],
                ),
              );
            } else {
              return Offstage();
            }
          }
        });
  }

  Widget _buildAddressBook(BuildContext context) {
    return Row(
      children: [
        Card(
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(20),
              side: BorderSide(color: MyTheme.grayBg)),
          child: Container(
            width: 200,
            height: double.infinity,
            padding: EdgeInsets.symmetric(horizontal: 12.0, vertical: 8.0),
            child: Column(
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text(translate('Tags')),
                    InkWell(
                      child: PopupMenuButton(
                          itemBuilder: (context) => [
                                PopupMenuItem(
                                  child: Text(translate("Add ID")),
                                  value: 'add-id',
                                ),
                                PopupMenuItem(
                                  child: Text(translate("Add Tag")),
                                  value: 'add-tag',
                                ),
                                PopupMenuItem(
                                  child: Text(translate("Unselect all tags")),
                                  value: 'unset-all-tag',
                                ),
                              ],
                          onSelected: handleAbOp,
                          child: Icon(Icons.more_vert_outlined)),
                    )
                  ],
                ),
                Expanded(
                  child: Container(
                    width: double.infinity,
                    height: double.infinity,
                    decoration: BoxDecoration(
                        border: Border.all(color: MyTheme.darkGray)),
                    child: Obx(
                      () => Wrap(
                        children: gFFI.abModel.tags
                            .map((e) => buildTag(e, gFFI.abModel.selectedTags,
                                    onTap: () {
                                  //
                                  if (gFFI.abModel.selectedTags.contains(e)) {
                                    gFFI.abModel.selectedTags.remove(e);
                                  } else {
                                    gFFI.abModel.selectedTags.add(e);
                                  }
                                }))
                            .toList(),
                      ),
                    ),
                  ).marginSymmetric(vertical: 8.0),
                )
              ],
            ),
          ),
        ).marginOnly(right: 8.0),
        Expanded(
          child: Align(
              alignment: Alignment.topLeft, child: AddressBookPeerWidget()),
        )
      ],
    );
  }

  Widget buildTag(String tagName, RxList<dynamic> rxTags, {Function()? onTap}) {
    return ContextMenuArea(
      width: 100,
      builder: (context) => [
        ListTile(
          title: Text(translate("Delete")),
          onTap: () {
            gFFI.abModel.deleteTag(tagName);
            gFFI.abModel.updateAb();
            Future.delayed(Duration.zero, () => Get.back());
          },
        )
      ],
      child: GestureDetector(
        onTap: onTap,
        child: Obx(
          () => Container(
            decoration: BoxDecoration(
                color: rxTags.contains(tagName) ? Colors.blue : null,
                border: Border.all(color: MyTheme.darkGray),
                borderRadius: BorderRadius.circular(10)),
            margin: EdgeInsets.symmetric(horizontal: 4.0, vertical: 8.0),
            padding: EdgeInsets.symmetric(vertical: 2.0, horizontal: 8.0),
            child: Text(
              tagName,
              style: TextStyle(
                  color: rxTags.contains(tagName) ? MyTheme.white : null),
            ),
          ),
        ),
      ),
    );
  }

  /// tag operation
  void handleAbOp(String value) {
    if (value == 'add-id') {
      abAddId();
    } else if (value == 'add-tag') {
      abAddTag();
    } else if (value == 'unset-all-tag') {
      gFFI.abModel.unsetSelectedTags();
    }
  }

  void abAddId() async {
    var field = "";
    var msg = "";
    var isInProgress = false;
    gFFI.dialogManager.show((setState, close) {
      return CustomAlertDialog(
        title: Text(translate("Add ID")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(translate("whitelist_sep")),
            SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    onChanged: (s) {
                      field = s;
                    },
                    maxLines: null,
                    decoration: InputDecoration(
                      border: OutlineInputBorder(),
                      errorText: msg.isEmpty ? null : translate(msg),
                    ),
                    controller: TextEditingController(text: field),
                  ),
                ),
              ],
            ),
            SizedBox(
              height: 4.0,
            ),
            Offstage(offstage: !isInProgress, child: LinearProgressIndicator())
          ],
        ),
        actions: [
          TextButton(
              onPressed: () {
                close();
              },
              child: Text(translate("Cancel"))),
          TextButton(
              onPressed: () async {
                setState(() {
                  msg = "";
                  isInProgress = true;
                });
                field = field.trim();
                if (field.isEmpty) {
                  // pass
                } else {
                  final ids = field.trim().split(RegExp(r"[\s,;\n]+"));
                  field = ids.join(',');
                  for (final newId in ids) {
                    if (gFFI.abModel.idContainBy(newId)) {
                      continue;
                    }
                    gFFI.abModel.addId(newId);
                  }
                  await gFFI.abModel.updateAb();
                  this.setState(() {});
                  // final currentPeers
                }
                close();
              },
              child: Text(translate("OK"))),
        ],
      );
    });
  }

  void abAddTag() async {
    var field = "";
    var msg = "";
    var isInProgress = false;
    gFFI.dialogManager.show((setState, close) {
      return CustomAlertDialog(
        title: Text(translate("Add Tag")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(translate("whitelist_sep")),
            SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    onChanged: (s) {
                      field = s;
                    },
                    maxLines: null,
                    decoration: InputDecoration(
                      border: OutlineInputBorder(),
                      errorText: msg.isEmpty ? null : translate(msg),
                    ),
                    controller: TextEditingController(text: field),
                  ),
                ),
              ],
            ),
            SizedBox(
              height: 4.0,
            ),
            Offstage(offstage: !isInProgress, child: LinearProgressIndicator())
          ],
        ),
        actions: [
          TextButton(
              onPressed: () {
                close();
              },
              child: Text(translate("Cancel"))),
          TextButton(
              onPressed: () async {
                setState(() {
                  msg = "";
                  isInProgress = true;
                });
                field = field.trim();
                if (field.isEmpty) {
                  // pass
                } else {
                  final tags = field.trim().split(RegExp(r"[\s,;\n]+"));
                  field = tags.join(',');
                  for (final tag in tags) {
                    gFFI.abModel.addTag(tag);
                  }
                  await gFFI.abModel.updateAb();
                  // final currentPeers
                }
                close();
              },
              child: Text(translate("OK"))),
        ],
      );
    });
  }

  void abEditTag(String id) {
    var isInProgress = false;

    final tags = List.of(gFFI.abModel.tags);
    var selectedTag = gFFI.abModel.getPeerTags(id).obs;

    gFFI.dialogManager.show((setState, close) {
      return CustomAlertDialog(
        title: Text(translate("Edit Tag")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              padding: EdgeInsets.symmetric(horizontal: 16.0, vertical: 8.0),
              child: Wrap(
                children: tags
                    .map((e) => buildTag(e, selectedTag, onTap: () {
                          if (selectedTag.contains(e)) {
                            selectedTag.remove(e);
                          } else {
                            selectedTag.add(e);
                          }
                        }))
                    .toList(growable: false),
              ),
            ),
            Offstage(offstage: !isInProgress, child: LinearProgressIndicator())
          ],
        ),
        actions: [
          TextButton(
              onPressed: () {
                close();
              },
              child: Text(translate("Cancel"))),
          TextButton(
              onPressed: () async {
                setState(() {
                  isInProgress = true;
                });
                gFFI.abModel.changeTagForPeer(id, selectedTag);
                await gFFI.abModel.updateAb();
                close();
              },
              child: Text(translate("OK"))),
        ],
      );
    });
  }
}

class WebMenu extends StatefulWidget {
  @override
  _WebMenuState createState() => _WebMenuState();
}

class _WebMenuState extends State<WebMenu> {
  String? username;
  String url = "";

  @override
  void initState() {
    super.initState();
    () async {
      final usernameRes = await getUsername();
      final urlRes = await getUrl();
      var update = false;
      if (usernameRes != username) {
        username = usernameRes;
        update = true;
      }
      if (urlRes != url) {
        url = urlRes;
        update = true;
      }

      if (update) {
        setState(() {});
      }
    }();
  }

  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    return PopupMenuButton<String>(
        icon: Icon(Icons.more_vert),
        itemBuilder: (context) {
          return (isIOS
                  ? [
                      PopupMenuItem(
                        child: Icon(Icons.qr_code_scanner, color: Colors.black),
                        value: "scan",
                      )
                    ]
                  : <PopupMenuItem<String>>[]) +
              [
                PopupMenuItem(
                  child: Text(translate('ID/Relay Server')),
                  value: "server",
                )
              ] +
              (url.contains('admin.rustdesk.com')
                  ? <PopupMenuItem<String>>[]
                  : [
                      PopupMenuItem(
                        child: Text(username == null
                            ? translate("Login")
                            : translate("Logout") + ' ($username)'),
                        value: "login",
                      )
                    ]) +
              [
                PopupMenuItem(
                  child: Text(translate('About') + ' RustDesk'),
                  value: "about",
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
            if (username == null) {
              showLogin(gFFI.dialogManager);
            } else {
              logout(gFFI.dialogManager);
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

class _PeerTabbedPage extends StatefulWidget {
  final List<String> tabs;
  final List<Widget> children;
  const _PeerTabbedPage({required this.tabs, required this.children, Key? key})
      : super(key: key);
  @override
  _PeerTabbedPageState createState() => _PeerTabbedPageState();
}

class _PeerTabbedPageState extends State<_PeerTabbedPage>
    with SingleTickerProviderStateMixin {
  late TabController _tabController;
  RxInt _tabIndex = 0.obs;

  @override
  void initState() {
    super.initState();
    _tabController =
        TabController(vsync: this, length: super.widget.tabs.length);
    _tabController.addListener(_handleTabSelection);
  }

  // hard code for now
  void _handleTabSelection() {
    if (_tabController.indexIsChanging) {
      _tabIndex.value = _tabController.index;
      switch (_tabController.index) {
        case 0:
          bind.mainLoadRecentPeers();
          break;
        case 1:
          bind.mainLoadFavPeers();
          break;
        case 2:
          bind.mainDiscover();
          break;
        case 3:
          break;
      }
    }
  }

  @override
  void dispose() {
    _tabController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // return DefaultTabController(
    //     length: 4,
    //     child: Column(
    //       crossAxisAlignment: CrossAxisAlignment.start,
    //       children: [
    //         _createTabBar(),
    //         _createTabBarView(),
    //       ],
    //     ));

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _createTabBar(context),
        _createTabBarView(),
      ],
    );
  }

  Widget _createTabBar(BuildContext context) {
    return TabBar(
        isScrollable: true,
        indicatorSize: TabBarIndicatorSize.label,
        indicatorColor: Colors.transparent,
        indicatorWeight: 0.1,
        controller: _tabController,
        labelPadding: EdgeInsets.zero,
        padding: EdgeInsets.only(left: 16),
        tabs: super.widget.tabs.asMap().entries.map((t) {
          return Obx(() => Container(
              padding: EdgeInsets.symmetric(horizontal: 8, vertical: 6),
              decoration: BoxDecoration(
                color:
                    _tabIndex.value == t.key ? MyTheme.color(context).bg : null,
                borderRadius: BorderRadius.circular(2),
              ),
              child: Text(
                t.value,
                style: TextStyle(
                    height: 1,
                    color: _tabIndex.value == t.key
                        ? MyTheme.color(context).text
                        : MyTheme.color(context).lightText),
              )));
        }).toList());
  }

  Widget _createTabBarView() {
    return Expanded(
        child: TabBarView(
                controller: _tabController, children: super.widget.children)
            .paddingSymmetric(horizontal: 12.0, vertical: 4.0));
  }
}
