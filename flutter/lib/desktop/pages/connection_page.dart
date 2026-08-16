// main window right pane

import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/connection_page_title.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';
import 'package:flutter_hbb/models/peer_model.dart';

import '../../common.dart';
import '../../common/formatter/id_formatter.dart';
import '../../common/widgets/peer_tab_page.dart';
import '../../common/widgets/autocomplete.dart';
import '../../models/platform_model.dart';
import '../../desktop/widgets/material_mod_popup_menu.dart' as mod_menu;

class OnlineStatusWidget extends StatefulWidget {
  const OnlineStatusWidget({Key? key, this.onSvcStatusChanged})
      : super(key: key);

  final VoidCallback? onSvcStatusChanged;

  @override
  State<OnlineStatusWidget> createState() => _OnlineStatusWidgetState();
}

/// State for the connection page.
class _OnlineStatusWidgetState extends State<OnlineStatusWidget> {
  final _svcStopped = Get.find<RxBool>(tag: 'stop-service');
  Timer? _updateTimer;

  double get em => 14.0;
  double? get height => bind.isIncomingOnly() ? null : em * 3;

  @override
  void initState() {
    super.initState();
    _updateTimer = periodic_immediate(Duration(seconds: 1), () async {
      updateStatus();
    });
  }

  @override
  void dispose() {
    _updateTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final isIncomingOnly = bind.isIncomingOnly();
    startServiceWidget() => Offstage(
          offstage: !_svcStopped.value,
          child: InkWell(
                  onTap: () async {
                    await start_service(true);
                  },
                  child: Text(translate("Start service"),
                      style: TextStyle(
                          decoration: TextDecoration.underline, fontSize: em)))
              .marginOnly(left: em),
        );

    basicWidget() => Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Container(
              height: 8,
              width: 8,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(4),
                color: _svcStopped.value ||
                        stateGlobal.svcStatus.value == SvcStatus.connecting
                    ? kColorWarn
                    : (stateGlobal.svcStatus.value == SvcStatus.ready
                        ? Color(0xFF178A67)
                        : Color(0xFFD95367)),
              ),
            ).marginSymmetric(horizontal: em),
            Container(
              width: isIncomingOnly ? 226 : null,
              child: _buildConnStatusMsg(),
            ),
            // stop
            if (!isIncomingOnly) startServiceWidget(),
          ],
        );

    return Container(
      height: height,
      child: Obx(() => isIncomingOnly
          ? Column(
              children: [
                basicWidget(),
                Align(
                        child: startServiceWidget(),
                        alignment: Alignment.centerLeft)
                    .marginOnly(top: 2.0, left: 22.0),
              ],
            )
          : basicWidget()),
    ).paddingOnly(right: isIncomingOnly ? 8 : 0);
  }

  _buildConnStatusMsg() {
    widget.onSvcStatusChanged?.call();
    return Text(
      _svcStopped.value
          ? translate("Service is not running")
          : stateGlobal.svcStatus.value == SvcStatus.connecting
              ? translate("Checking connection")
              : stateGlobal.svcStatus.value == SvcStatus.notReady
                  ? translate("Offline")
                  : translate('Online'),
      style: TextStyle(fontSize: em),
    );
  }

  updateStatus() async {
    final status =
        jsonDecode(await bind.mainGetConnectStatus()) as Map<String, dynamic>;
    final statusNum = status['status_num'] as int;
    if (statusNum == 0) {
      stateGlobal.svcStatus.value = SvcStatus.connecting;
    } else if (statusNum == -1) {
      stateGlobal.svcStatus.value = SvcStatus.notReady;
    } else if (statusNum == 1) {
      stateGlobal.svcStatus.value = SvcStatus.ready;
    } else {
      stateGlobal.svcStatus.value = SvcStatus.notReady;
    }
    try {
      stateGlobal.videoConnCount.value = status['video_conn_count'] as int;
    } catch (_) {}
  }
}

/// Connection page for connecting to a remote peer.
class ConnectionPage extends StatefulWidget {
  const ConnectionPage({Key? key}) : super(key: key);

  static _ConnectionPageState? _activeState;

  /// Focus the primary connection field from shell actions such as the
  /// sidebar's "Connect to device" command.
  static void focusRemoteId() {
    _activeState?._focusRemoteId();
  }

  @override
  State<ConnectionPage> createState() => _ConnectionPageState();
}

/// State for the connection page.
class _ConnectionPageState extends State<ConnectionPage>
    with SingleTickerProviderStateMixin, WindowListener {
  /// Controller for the id input bar.
  final _idController = IDTextEditingController();

  final RxBool _idInputFocused = false.obs;
  final FocusNode _idFocusNode = FocusNode();
  final TextEditingController _idEditingController = TextEditingController();

  String selectedConnectionType = 'Connect';

  bool isWindowMinimized = false;

  final AllPeersLoader _allPeersLoader = AllPeersLoader();

  // https://github.com/flutter/flutter/issues/157244
  Iterable<Peer> _autocompleteOpts = [];

  final _menuOpen = false.obs;

  void _focusRemoteId() {
    if (!mounted) return;
    _idFocusNode.requestFocus();
    final textLength = _idEditingController.value.text.length;
    _idEditingController.selection =
        TextSelection(baseOffset: 0, extentOffset: textLength);
  }

  @override
  void initState() {
    super.initState();
    ConnectionPage._activeState = this;
    _allPeersLoader.init(setState);
    _idFocusNode.addListener(onFocusChanged);
    if (_idController.text.isEmpty) {
      WidgetsBinding.instance.addPostFrameCallback((_) async {
        final lastRemoteId = await bind.mainGetLastRemoteId();
        if (lastRemoteId != _idController.id) {
          setState(() {
            _idController.id = lastRemoteId;
          });
        }
      });
    }
    Get.put<TextEditingController>(_idEditingController);
    Get.put<IDTextEditingController>(_idController);
    windowManager.addListener(this);
  }

  @override
  void dispose() {
    if (identical(ConnectionPage._activeState, this)) {
      ConnectionPage._activeState = null;
    }
    _idController.dispose();
    windowManager.removeListener(this);
    _allPeersLoader.clear();
    _idFocusNode.removeListener(onFocusChanged);
    _idFocusNode.dispose();
    _idEditingController.dispose();
    if (Get.isRegistered<IDTextEditingController>()) {
      Get.delete<IDTextEditingController>();
    }
    if (Get.isRegistered<TextEditingController>()) {
      Get.delete<TextEditingController>();
    }
    super.dispose();
  }

  @override
  void onWindowEvent(String eventName) {
    super.onWindowEvent(eventName);
    if (eventName == 'minimize') {
      isWindowMinimized = true;
    } else if (eventName == 'maximize' || eventName == 'restore') {
      if (isWindowMinimized && isWindows) {
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
    stateGlobal.resizeEdgeSize.value = stateGlobal.isMaximized.isTrue
        ? kMaximizeEdgeSize
        : windowResizeEdgeSize;
  }

  @override
  void onWindowClose() {
    super.onWindowClose();
    bind.mainOnMainWindowClose();
  }

  void onFocusChanged() {
    _idInputFocused.value = _idFocusNode.hasFocus;
    if (_idFocusNode.hasFocus) {
      if (_allPeersLoader.needLoad) {
        _allPeersLoader.getAllPeers();
      }

      final textLength = _idEditingController.value.text.length;
      // Select all to facilitate removing text, just following the behavior of address input of chrome.
      _idEditingController.selection =
          TextSelection(baseOffset: 0, extentOffset: textLength);
    }
  }

  @override
  Widget build(BuildContext context) {
    final isOutgoingOnly = bind.isOutgoingOnly();
    return Container(
      color: Theme.of(context).scaffoldBackgroundColor,
      child: Column(
        children: [
          Expanded(
              child: Column(
            children: [
              Row(
                children: [
                  Flexible(child: _buildRemoteIDTextField(context)),
                ],
              ).marginOnly(top: 24),
              const SizedBox(height: 16),
              Divider(color: MyTheme.border).paddingOnly(right: 12),
              Expanded(child: PeerTabPage()),
            ],
          ).paddingOnly(left: 12.0)),
          if (!isOutgoingOnly) Divider(height: 1, color: MyTheme.border),
          if (!isOutgoingOnly) OnlineStatusWidget()
        ],
      ),
    );
  }

  /// Callback for the connect button.
  /// Connects to the selected peer.
  void onConnect(
      {bool isFileTransfer = false,
      bool isViewCamera = false,
      bool isTerminal = false}) {
    var id = _idController.id;
    connect(context, id,
        isFileTransfer: isFileTransfer,
        isViewCamera: isViewCamera,
        isTerminal: isTerminal);
  }

  /// UI for the remote ID TextField.
  /// Search for a peer.
  Widget _buildRemoteIDTextField(BuildContext context) {
    var w = Container(
      width: 320 + 20 * 2,
      padding: const EdgeInsets.fromLTRB(0, 18, 0, 14),
      decoration: BoxDecoration(
        border: Border(
          top: BorderSide(color: MyTheme.accent.withOpacity(0.35)),
          bottom: BorderSide(color: MyTheme.border),
        ),
      ),
      child: Ink(
        child: Column(
          children: [
            getConnectionPageTitle(context, false).marginOnly(bottom: 12),
            Row(
              children: [
                Expanded(
                    child: RawAutocomplete<Peer>(
                  optionsBuilder: (TextEditingValue textEditingValue) {
                    if (textEditingValue.text == '') {
                      _autocompleteOpts = const Iterable<Peer>.empty();
                    } else if (_allPeersLoader.peers.isEmpty &&
                        !_allPeersLoader.isPeersLoaded) {
                      Peer emptyPeer = Peer(
                        id: '',
                        username: '',
                        hostname: '',
                        alias: '',
                        platform: '',
                        tags: [],
                        hash: '',
                        password: '',
                        forceAlwaysRelay: false,
                        rdpPort: '',
                        rdpUsername: '',
                        loginName: '',
                        device_group_name: '',
                        note: '',
                      );
                      _autocompleteOpts = [emptyPeer];
                    } else {
                      String textWithoutSpaces =
                          textEditingValue.text.replaceAll(" ", "");
                      if (int.tryParse(textWithoutSpaces) != null) {
                        textEditingValue = TextEditingValue(
                          text: textWithoutSpaces,
                          selection: textEditingValue.selection,
                        );
                      }
                      String textToFind = textEditingValue.text.toLowerCase();
                      _autocompleteOpts = _allPeersLoader.peers
                          .where((peer) =>
                              peer.id.toLowerCase().contains(textToFind) ||
                              peer.username
                                  .toLowerCase()
                                  .contains(textToFind) ||
                              peer.hostname
                                  .toLowerCase()
                                  .contains(textToFind) ||
                              peer.alias.toLowerCase().contains(textToFind))
                          .toList();
                      _allPeersLoader.queryOnlines(_autocompleteOpts);
                    }
                    return _autocompleteOpts;
                  },
                  focusNode: _idFocusNode,
                  textEditingController: _idEditingController,
                  fieldViewBuilder: (
                    BuildContext context,
                    TextEditingController fieldTextEditingController,
                    FocusNode fieldFocusNode,
                    VoidCallback onFieldSubmitted,
                  ) {
                    updateTextAndPreserveSelection(
                        fieldTextEditingController, _idController.text);
                    return Obx(() => TextField(
                          autocorrect: false,
                          enableSuggestions: false,
                          keyboardType: TextInputType.visiblePassword,
                          focusNode: fieldFocusNode,
                          style: const TextStyle(
                            fontSize: 22,
                            fontWeight: FontWeight.w500,
                            height: 1.4,
                          ),
                          maxLines: 1,
                          cursorColor: MyTheme.accent,
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
                          onSubmitted: (_) {
                            onConnect();
                          },
                        ).workaroundFreezeLinuxMint());
                  },
                  onSelected: (option) {
                    setState(() {
                      _idController.id = option.id;
                      FocusScope.of(context).unfocus();
                    });
                  },
                  optionsViewBuilder: (BuildContext context,
                      AutocompleteOnSelected<Peer> onSelected,
                      Iterable<Peer> options) {
                    options = _autocompleteOpts;
                    double maxHeight = options.length * 50;
                    if (options.length == 1) {
                      maxHeight = 52;
                    } else if (options.length == 3) {
                      maxHeight = 146;
                    } else if (options.length == 4) {
                      maxHeight = 193;
                    }
                    maxHeight = maxHeight.clamp(0, 200);

                    return Align(
                      alignment: Alignment.topLeft,
                      child: Container(
                          decoration: BoxDecoration(
                            boxShadow: [
                              BoxShadow(
                                color: Colors.black.withOpacity(0.3),
                                blurRadius: 5,
                                spreadRadius: 1,
                              ),
                            ],
                          ),
                          child: ClipRRect(
                              borderRadius: BorderRadius.circular(5),
                              child: Material(
                                elevation: 4,
                                child: ConstrainedBox(
                                  constraints: BoxConstraints(
                                    maxHeight: maxHeight,
                                    maxWidth: 319,
                                  ),
                                  child: _allPeersLoader.peers.isEmpty &&
                                          !_allPeersLoader.isPeersLoaded
                                      ? Container(
                                          height: 80,
                                          child: Center(
                                            child: CircularProgressIndicator(
                                              strokeWidth: 2,
                                            ),
                                          ))
                                      : Padding(
                                          padding:
                                              const EdgeInsets.only(top: 5),
                                          child: ListView(
                                            children: options
                                                .map((peer) =>
                                                    AutocompletePeerTile(
                                                        onSelect: () =>
                                                            onSelected(peer),
                                                        peer: peer))
                                                .toList(),
                                          ),
                                        ),
                                ),
                              ))),
                    );
                  },
                )),
              ],
            ),
            Padding(
              padding: const EdgeInsets.only(top: 13.0),
              child: Row(mainAxisAlignment: MainAxisAlignment.end, children: [
                SizedBox(
                  height: 38.0,
                  child: ElevatedButton(
                    onPressed: () {
                      onConnect();
                    },
                    child: Text(translate("Connect")),
                  ),
                ),
                const SizedBox(width: 8),
                Container(
                  height: 38.0,
                  width: 38.0,
                  decoration: BoxDecoration(
                    border: Border.all(color: Theme.of(context).dividerColor),
                    borderRadius: BorderRadius.zero,
                  ),
                  child: Center(
                    child: StatefulBuilder(
                      builder: (context, setState) {
                        var offset = Offset(0, 0);
                        return Obx(() => InkWell(
                              child: _menuOpen.value
                                  ? Transform.rotate(
                                      angle: pi,
                                      child: Icon(IconFont.more, size: 14),
                                    )
                                  : Icon(IconFont.more, size: 14),
                              onTapDown: (e) {
                                offset = e.globalPosition;
                              },
                              onTap: () async {
                                _menuOpen.value = true;
                                final x = offset.dx;
                                final y = offset.dy;
                                await mod_menu
                                    .showMenu(
                                  context: context,
                                  position: RelativeRect.fromLTRB(x, y, x, y),
                                  items: [
                                    (
                                      'Transfer file',
                                      () => onConnect(isFileTransfer: true)
                                    ),
                                    (
                                      'View camera',
                                      () => onConnect(isViewCamera: true)
                                    ),
                                    (
                                      '${translate('Terminal')} (beta)',
                                      () => onConnect(isTerminal: true)
                                    ),
                                  ]
                                      .map((e) => MenuEntryButton<String>(
                                            childBuilder: (TextStyle? style) =>
                                                Text(
                                              translate(e.$1),
                                              style: style,
                                            ),
                                            proc: () => e.$2(),
                                            padding: EdgeInsets.symmetric(
                                                horizontal:
                                                    kDesktopMenuPadding.left),
                                            dismissOnClicked: true,
                                          ))
                                      .map((e) => e.build(
                                          context,
                                          const MenuConfig(
                                              commonColor: CustomPopupMenuTheme
                                                  .commonColor,
                                              height:
                                                  CustomPopupMenuTheme.height,
                                              dividerHeight:
                                                  CustomPopupMenuTheme
                                                      .dividerHeight)))
                                      .expand((i) => i)
                                      .toList(),
                                  elevation: 8,
                                )
                                    .then((_) {
                                  _menuOpen.value = false;
                                });
                              },
                            ));
                      },
                    ),
                  ),
                ),
              ]),
            ),
          ],
        ),
      ),
    );
    return Container(
        constraints: const BoxConstraints(maxWidth: 600), child: w);
  }
}
