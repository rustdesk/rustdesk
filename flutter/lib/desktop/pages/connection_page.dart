import 'dart:async';
import 'dart:convert';

import 'package:contextmenu/contextmenu.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/widgets/peer_widget.dart';
import 'package:flutter_hbb/desktop/widgets/peercard_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher_string.dart';

import '../../common.dart';
import '../../common/formatter/id_formatter.dart';
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
                      getSearchBarUI(context),
                    ],
                  ).marginOnly(top: 22),
                  SizedBox(height: 12),
                  Divider(),
                  Expanded(
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
                      FutureBuilder<Widget>(
                          future: buildAddressBook(context),
                          builder: (context, snapshot) {
                            if (snapshot.hasData) {
                              return snapshot.data!;
                            } else {
                              return const Offstage();
                            }
                          }),
                    ],
                  )),
                ],
              ).marginSymmetric(horizontal: 22),
            ),
            const Divider(),
            SizedBox(height: 50, child: Obx(() => buildStatus()))
                .paddingSymmetric(horizontal: 12.0)
          ]),
    );
  }

  /// Callback for the connect button.
  /// Connects to the selected peer.
  void onConnect({bool isFileTransfer = false}) {
    final id = _idController.id;
    connect(id, isFileTransfer: isFileTransfer);
  }

  /// Connect to a peer with [id].
  /// If [isFileTransfer], starts a session only for file transfer.
  void connect(String id, {bool isFileTransfer = false}) async {
    if (id == '') return;
    id = id.replaceAll(' ', '');
    if (isFileTransfer) {
      await rustDeskWinManager.newFileTransfer(id);
    } else {
      await rustDeskWinManager.newRemoteDesktop(id);
    }
    FocusScopeNode currentFocus = FocusScope.of(context);
    if (!currentFocus.hasPrimaryFocus) {
      currentFocus.unfocus();
    }
  }

  /// UI for the search bar.
  /// Search for a peer and connect to it if the id exists.
  Widget getSearchBarUI(BuildContext context) {
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
                          height: 24,
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
                      child: Container(
                        height: 24,
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
                                fontSize: 12, color: MyTheme.color(context).bg),
                          ),
                        ).marginSymmetric(horizontal: 12),
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
              onPressed: () async {
                bool checked = await bind.mainCheckSuperUserPermission();
                if (checked) {
                  bind.mainSetOption(key: "stop-service", value: "");
                }
              },
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
        Text(translate('Ready')),
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
              return const Center(
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
    TextEditingController controller = TextEditingController(text: field);

    gFFI.dialogManager.show((setState, close) {
      submit() async {
        setState(() {
          msg = "";
          isInProgress = true;
        });
        field = controller.text.trim();
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
      }

      return CustomAlertDialog(
        title: Text(translate("Add ID")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(translate("whitelist_sep")),
            const SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                Expanded(
                  child: TextField(
                      maxLines: null,
                      decoration: InputDecoration(
                        border: const OutlineInputBorder(),
                        errorText: msg.isEmpty ? null : translate(msg),
                      ),
                      controller: controller,
                      focusNode: FocusNode()..requestFocus()),
                ),
              ],
            ),
            const SizedBox(
              height: 4.0,
            ),
            Offstage(
                offstage: !isInProgress, child: const LinearProgressIndicator())
          ],
        ),
        actions: [
          TextButton(onPressed: close, child: Text(translate("Cancel"))),
          TextButton(onPressed: submit, child: Text(translate("OK"))),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
  }

  void abAddTag() async {
    var field = "";
    var msg = "";
    var isInProgress = false;
    TextEditingController controller = TextEditingController(text: field);
    gFFI.dialogManager.show((setState, close) {
      submit() async {
        setState(() {
          msg = "";
          isInProgress = true;
        });
        field = controller.text.trim();
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
      }

      return CustomAlertDialog(
        title: Text(translate("Add Tag")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(translate("whitelist_sep")),
            const SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    maxLines: null,
                    decoration: InputDecoration(
                      border: const OutlineInputBorder(),
                      errorText: msg.isEmpty ? null : translate(msg),
                    ),
                    controller: controller,
                    focusNode: FocusNode()..requestFocus(),
                  ),
                ),
              ],
            ),
            const SizedBox(
              height: 4.0,
            ),
            Offstage(
                offstage: !isInProgress, child: const LinearProgressIndicator())
          ],
        ),
        actions: [
          TextButton(onPressed: close, child: Text(translate("Cancel"))),
          TextButton(onPressed: submit, child: Text(translate("OK"))),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
  }

  void abEditTag(String id) {
    var isInProgress = false;

    final tags = List.of(gFFI.abModel.tags);
    var selectedTag = gFFI.abModel.getPeerTags(id).obs;

    gFFI.dialogManager.show((setState, close) {
      submit() async {
        setState(() {
          isInProgress = true;
        });
        gFFI.abModel.changeTagForPeer(id, selectedTag);
        await gFFI.abModel.updateAb();
        close();
      }

      return CustomAlertDialog(
        title: Text(translate("Edit Tag")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              padding:
                  const EdgeInsets.symmetric(horizontal: 16.0, vertical: 8.0),
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
            Offstage(
                offstage: !isInProgress, child: const LinearProgressIndicator())
          ],
        ),
        actions: [
          TextButton(onPressed: close, child: Text(translate("Cancel"))),
          TextButton(onPressed: submit, child: Text(translate("OK"))),
        ],
        onSubmit: submit,
        onCancel: close,
      );
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
  late PageController _pageController = PageController();
  RxInt _tabIndex = 0.obs;

  @override
  void initState() {
    super.initState();
  }

  // hard code for now
  void _handleTabSelection(int index) {
    // reset search text
    peerSearchText.value = "";
    peerSearchTextController.clear();
    _tabIndex.value = index;
    _pageController.jumpToPage(index);
    switch (index) {
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
        gFFI.abModel.updateAb();
        break;
    }
  }

  @override
  void dispose() {
    _pageController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      textBaseline: TextBaseline.ideographic,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          height: 28,
          child: Row(
            children: [
              Expanded(child: _createTabBar(context)),
              _createSearchBar(context),
              _createPeerViewTypeSwitch(context),
            ],
          ),
        ),
        _createTabBarView(),
      ],
    );
  }

  Widget _createTabBar(BuildContext context) {
    return ListView(
        scrollDirection: Axis.horizontal,
        shrinkWrap: true,
        controller: ScrollController(),
        children: super.widget.tabs.asMap().entries.map((t) {
          return Obx(() => GestureDetector(
                child: Container(
                    padding: EdgeInsets.symmetric(horizontal: 8),
                    decoration: BoxDecoration(
                      color: _tabIndex.value == t.key
                          ? MyTheme.color(context).bg
                          : null,
                      borderRadius: BorderRadius.circular(2),
                    ),
                    child: Align(
                      alignment: Alignment.center,
                      child: Text(
                        t.value,
                        textAlign: TextAlign.center,
                        style: TextStyle(
                            height: 1,
                            fontSize: 14,
                            color: _tabIndex.value == t.key
                                ? MyTheme.color(context).text
                                : MyTheme.color(context).lightText),
                      ),
                    )),
                onTap: () => _handleTabSelection(t.key),
              ));
        }).toList());
  }

  Widget _createTabBarView() {
    return Expanded(
        child: PageView(
                controller: _pageController, children: super.widget.children)
            .marginSymmetric(vertical: 12));
  }

  _createSearchBar(BuildContext context) {
    RxBool focused = false.obs;
    FocusNode focusNode = FocusNode();
    focusNode.addListener(() => focused.value = focusNode.hasFocus);
    RxBool rowHover = false.obs;
    RxBool clearHover = false.obs;
    return Container(
      width: 120,
      height: 25,
      margin: EdgeInsets.only(right: 13),
      decoration: BoxDecoration(color: MyTheme.color(context).bg),
      child: Obx(() => Row(
            children: [
              Expanded(
                child: MouseRegion(
                  onEnter: (_) => rowHover.value = true,
                  onExit: (_) => rowHover.value = false,
                  child: Row(
                    children: [
                      Icon(
                        IconFont.search,
                        size: 16,
                        color: MyTheme.color(context).placeholder,
                      ).marginSymmetric(horizontal: 4),
                      Expanded(
                        child: TextField(
                          controller: peerSearchTextController,
                          onChanged: (searchText) {
                            peerSearchText.value = searchText;
                          },
                          focusNode: focusNode,
                          textAlign: TextAlign.start,
                          maxLines: 1,
                          cursorColor: MyTheme.color(context).lightText,
                          cursorHeight: 18,
                          cursorWidth: 1,
                          style: TextStyle(fontSize: 14),
                          decoration: InputDecoration(
                            contentPadding: EdgeInsets.symmetric(vertical: 6),
                            hintText:
                                focused.value ? null : translate("Search ID"),
                            hintStyle: TextStyle(
                                fontSize: 14,
                                color: MyTheme.color(context).placeholder),
                            border: InputBorder.none,
                            isDense: true,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
              Offstage(
                offstage: !(peerSearchText.value.isNotEmpty &&
                    (rowHover.value || clearHover.value)),
                child: InkWell(
                    onHover: (value) => clearHover.value = value,
                    child: Icon(
                      IconFont.round_close,
                      size: 16,
                      color: clearHover.value
                          ? MyTheme.color(context).text
                          : MyTheme.color(context).placeholder,
                    ).marginSymmetric(horizontal: 4),
                    onTap: () {
                      peerSearchTextController.clear();
                      peerSearchText.value = "";
                    }),
              )
            ],
          )),
    );
  }

  _createPeerViewTypeSwitch(BuildContext context) {
    final activeDeco = BoxDecoration(color: MyTheme.color(context).bg);
    return Row(
      children: [
        Obx(
          () => Container(
            padding: EdgeInsets.all(4.0),
            decoration:
                peerCardUiType.value == PeerUiType.grid ? activeDeco : null,
            child: InkWell(
                onTap: () {
                  peerCardUiType.value = PeerUiType.grid;
                },
                child: Icon(
                  Icons.grid_view_rounded,
                  size: 18,
                  color: peerCardUiType.value == PeerUiType.grid
                      ? MyTheme.color(context).text
                      : MyTheme.color(context).lightText,
                )),
          ),
        ),
        Obx(
          () => Container(
            padding: EdgeInsets.all(4.0),
            decoration:
                peerCardUiType.value == PeerUiType.list ? activeDeco : null,
            child: InkWell(
                onTap: () {
                  peerCardUiType.value = PeerUiType.list;
                },
                child: Icon(
                  Icons.list,
                  size: 18,
                  color: peerCardUiType.value == PeerUiType.list
                      ? MyTheme.color(context).text
                      : MyTheme.color(context).lightText,
                )),
          ),
        ),
      ],
    );
  }
}
