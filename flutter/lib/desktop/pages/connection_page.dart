// main window right pane

import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/widgets/connection_page_title.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher_string.dart';
import 'package:window_manager/window_manager.dart';
import 'package:flutter_hbb/models/peer_model.dart';

import '../../common.dart';
import '../../common/formatter/id_formatter.dart';
import '../../common/widgets/peer_tab_page.dart';
import '../../common/widgets/autocomplete.dart';
import '../../models/platform_model.dart';
import '../../desktop/widgets/material_mod_popup_menu.dart' as mod_menu;

class NetworkModeInfo {
  final String mode;
  final String label;
  final String detail;
  final String trustPhrase;
  final List<String> directEndpoints;
  final bool pairingRequired;

  const NetworkModeInfo({
    required this.mode,
    required this.label,
    required this.detail,
    required this.trustPhrase,
    required this.directEndpoints,
    required this.pairingRequired,
  });

  const NetworkModeInfo.fallback()
      : mode = 'not_configured',
        label = 'Offline',
        detail = '',
        trustPhrase = '',
        directEndpoints = const [],
        pairingRequired = false;

  factory NetworkModeInfo.fromJson(Map<String, dynamic> json) {
    return NetworkModeInfo(
      mode: json['mode'] as String? ?? 'not_configured',
      label: json['label'] as String? ?? 'Offline',
      detail: json['detail'] as String? ?? '',
      trustPhrase: json['trust_phrase'] as String? ?? '',
      directEndpoints: (json['direct_endpoints'] as List<dynamic>? ?? const [])
          .map((e) => e.toString())
          .where((e) => e.isNotEmpty)
          .toList(),
      pairingRequired: json['pairing_required'] as bool? ?? false,
    );
  }
}

Future<NetworkModeInfo> loadNetworkModeInfo() async {
  Future<NetworkModeInfo> fallbackFromOptions({String trustPhrase = ''}) async {
    final usingPublicServer = await bind.mainIsUsingPublicServer();
    final apiServer = await bind.mainGetApiServer();
    final rendezvousServer =
        await bind.mainGetOption(key: 'custom-rendezvous-server');
    final relayServer = await bind.mainGetOption(key: 'relay-server');
    final directAccessEnabled = option2bool(kOptionDirectServer,
        await bind.mainGetOption(key: kOptionDirectServer));
    final pairingRequired =
        (await bind.mainGetOption(key: kOptionDirectAccessPairingPassphrase))
            .isNotEmpty;
    final detail = rendezvousServer.isNotEmpty
        ? rendezvousServer
        : relayServer.isNotEmpty
            ? relayServer
            : apiServer;
    if (usingPublicServer) {
      return NetworkModeInfo(
        mode: 'public_server',
        label: 'Public Server',
        detail: detail,
        trustPhrase: trustPhrase,
        directEndpoints: const [],
        pairingRequired: pairingRequired,
      );
    }
    if (detail.isNotEmpty) {
      return NetworkModeInfo(
        mode: 'private_server',
        label: 'Private Server',
        detail: detail,
        trustPhrase: trustPhrase,
        directEndpoints: const [],
        pairingRequired: pairingRequired,
      );
    }
    if (directAccessEnabled) {
      return NetworkModeInfo(
        mode: 'local_only',
        label: 'Local Only',
        detail: '',
        trustPhrase: trustPhrase,
        directEndpoints: const [],
        pairingRequired: pairingRequired,
      );
    }
    return NetworkModeInfo(
      mode: 'not_configured',
      label: 'Offline',
      detail: '',
      trustPhrase: trustPhrase,
      directEndpoints: const [],
      pairingRequired: pairingRequired,
    );
  }

  try {
    final raw = await bind.mainGetCommon(key: 'network-mode-info');
    if (raw.isEmpty) {
      return fallbackFromOptions();
    }
    final info =
        NetworkModeInfo.fromJson(jsonDecode(raw) as Map<String, dynamic>);
    if (info.mode == 'not_configured') {
      return fallbackFromOptions(trustPhrase: info.trustPhrase);
    }
    return info;
  } catch (_) {
    return fallbackFromOptions();
  }
}

Future<void> copyNetworkStatusValue(String value) async {
  if (value.isEmpty) {
    return;
  }
  await Clipboard.setData(ClipboardData(text: value));
  showToast('$value\n${translate("Copied")}');
}

Color colorForNetworkMode(String mode) {
  switch (mode) {
    case 'local_only':
      return const Color(0xFF2E8B57);
    case 'private_server':
      return const Color(0xFF2F65BA);
    case 'public_server':
      return const Color(0xFFF39C12);
    case 'not_configured':
    default:
      return Colors.grey;
  }
}

class NetworkStatusPanel extends StatelessWidget {
  const NetworkStatusPanel({super.key});

  @override
  Widget build(BuildContext context) {
    return Obx(() {
      final mode = stateGlobal.networkMode.value;
      final label = stateGlobal.networkModeLabel.value;
      final detail = stateGlobal.networkModeDetail.value;
      final trustPhrase = stateGlobal.networkModeTrustPhrase.value;
      final directEndpoints =
          stateGlobal.networkModeDirectEndpoints.toList(growable: false);
      final pairingRequired = stateGlobal.networkModePairingRequired.value;
      final directAccessValue = directEndpoints.join(', ');
      final color = colorForNetworkMode(mode);
      final secondaryTextColor =
          Theme.of(context).textTheme.bodySmall?.color?.withOpacity(0.75);
      Widget buildStatusLine(
        String label,
        String value, {
        bool copyable = false,
        bool ellipsize = false,
      }) {
        return Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: Text(
                '$label: $value',
                maxLines: ellipsize ? 1 : null,
                overflow:
                    ellipsize ? TextOverflow.ellipsis : TextOverflow.visible,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: secondaryTextColor,
                    ),
              ),
            ),
            if (copyable)
              IconButton(
                onPressed: () => copyNetworkStatusValue(value),
                icon: const Icon(Icons.copy_rounded, size: 15),
                visualDensity: VisualDensity.compact,
                splashRadius: 16,
                constraints: const BoxConstraints(
                  minWidth: 24,
                  minHeight: 24,
                ),
                tooltip: 'Copy',
                padding: EdgeInsets.zero,
              ),
          ],
        );
      }

      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
            decoration: BoxDecoration(
              color: color.withOpacity(0.14),
              borderRadius: BorderRadius.circular(6),
              border: Border.all(color: color.withOpacity(0.4)),
            ),
            child: Text(
              label,
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: color,
                    fontWeight: FontWeight.w700,
                  ),
            ),
          ),
          const SizedBox(height: 8),
          buildStatusLine(
            'LAN discovery',
            translate(stateGlobal.lanDiscoveryModeLabel.value),
          ),
          if (trustPhrase.isNotEmpty) ...[
            const SizedBox(height: 4),
            buildStatusLine('Trust phrase', trustPhrase, copyable: true),
          ],
          if (mode == 'local_only' || pairingRequired) ...[
            const SizedBox(height: 4),
            buildStatusLine(
              'Pairing passphrase',
              pairingRequired ? 'Required' : 'Disabled',
            ),
          ],
          if (directAccessValue.isNotEmpty) ...[
            const SizedBox(height: 4),
            buildStatusLine(
              'Direct access',
              directAccessValue,
              copyable: true,
            ),
          ],
          if (detail.isNotEmpty) ...[
            const SizedBox(height: 4),
            buildStatusLine('Endpoint', detail, ellipsize: true),
          ],
          const SizedBox(height: 14),
          Divider(height: 1),
        ],
      );
    });
  }
}

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
  final _svcIsUsingPublicServer = true.obs;
  Timer? _updateTimer;

  double get em => 14.0;
  double? get height => bind.isIncomingOnly() ? null : em * 3;

  void onUsePublicServerGuide() {
    const url = "https://rustdesk.com/pricing";
    canLaunchUrlString(url).then((can) {
      if (can) {
        launchUrlString(url);
      }
    });
  }

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
    Color statusColor() {
      if (_svcStopped.value) {
        return kColorWarn;
      }
      if (stateGlobal.svcStatus.value == SvcStatus.ready) {
        return const Color.fromARGB(255, 50, 190, 166);
      }
      if (stateGlobal.svcStatus.value == SvcStatus.connecting) {
        return colorForNetworkMode(stateGlobal.networkMode.value);
      }
      return const Color.fromARGB(255, 224, 79, 95);
    }

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

    setupServerWidget() => Flexible(
          child: Offstage(
            offstage: !(!_svcStopped.value &&
                stateGlobal.svcStatus.value == SvcStatus.ready &&
                _svcIsUsingPublicServer.value),
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
        );

    basicWidget() => Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Container(
              height: 8,
              width: 8,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(4),
                color: statusColor(),
              ),
            ).marginSymmetric(horizontal: em),
            Container(
              width: isIncomingOnly ? 226 : null,
              child: _buildConnStatusMsg(),
            ),
            // stop
            if (!isIncomingOnly) startServiceWidget(),
            // ready && public
            // No need to show the guide if is custom client.
            if (!isIncomingOnly) setupServerWidget(),
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
    final statusText = _svcStopped.value
        ? translate("Service is not running")
        : stateGlobal.svcStatus.value == SvcStatus.connecting
            ? (stateGlobal.networkMode.value == 'local_only' ||
                    stateGlobal.networkMode.value == 'not_configured'
                ? stateGlobal.networkModeLabel.value
                : translate("connecting_status"))
            : stateGlobal.svcStatus.value == SvcStatus.notReady
                ? translate("not_ready_status")
                : translate('Ready');
    return Text(
      statusText,
      style: TextStyle(fontSize: em),
    );
  }

  updateStatus() async {
    final networkMode = await loadNetworkModeInfo();
    stateGlobal.networkMode.value = networkMode.mode;
    stateGlobal.networkModeLabel.value = networkMode.label;
    stateGlobal.networkModeDetail.value = networkMode.detail;
    stateGlobal.networkModeTrustPhrase.value = networkMode.trustPhrase;
    stateGlobal.networkModeDirectEndpoints
        .assignAll(networkMode.directEndpoints);
    stateGlobal.networkModePairingRequired.value = networkMode.pairingRequired;
    final lanDiscoveryMode = await loadLanDiscoveryMode();
    stateGlobal.lanDiscoveryMode.value = lanDiscoveryMode;
    stateGlobal.lanDiscoveryModeLabel.value =
        lanDiscoveryModeLabel(lanDiscoveryMode);
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
    _svcIsUsingPublicServer.value = await bind.mainIsUsingPublicServer();
    try {
      stateGlobal.videoConnCount.value = status['video_conn_count'] as int;
    } catch (_) {}
  }
}

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

  final RxBool _idInputFocused = false.obs;
  final FocusNode _idFocusNode = FocusNode();
  final TextEditingController _idEditingController = TextEditingController();

  String selectedConnectionType = 'Connect';

  bool isWindowMinimized = false;

  final AllPeersLoader _allPeersLoader = AllPeersLoader();

  // https://github.com/flutter/flutter/issues/157244
  Iterable<Peer> _autocompleteOpts = [];

  final _menuOpen = false.obs;

  @override
  void initState() {
    super.initState();
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
        if (!isOutgoingOnly) const Divider(height: 1),
        if (!isOutgoingOnly) OnlineStatusWidget()
      ],
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
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 22),
      decoration: BoxDecoration(
          borderRadius: const BorderRadius.all(Radius.circular(13)),
          border: Border.all(color: Theme.of(context).colorScheme.background)),
      child: Ink(
        child: Column(
          children: [
            getConnectionPageTitle(context, false).marginOnly(bottom: 15),
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
                              peer.discoveryEndpoint
                                  .toLowerCase()
                                  .contains(textToFind) ||
                              peer.discoveryTrustPhrase
                                  .toLowerCase()
                                  .contains(textToFind) ||
                              peer.alias.toLowerCase().contains(textToFind))
                          .toList();
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
                            fontFamily: 'WorkSans',
                            fontSize: 22,
                            height: 1.4,
                          ),
                          maxLines: 1,
                          cursorColor:
                              Theme.of(context).textTheme.titleLarge?.color,
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
                  height: 28.0,
                  child: ElevatedButton(
                    onPressed: () {
                      onConnect();
                    },
                    child: Text(translate("Connect")),
                  ),
                ),
                const SizedBox(width: 8),
                Container(
                  height: 28.0,
                  width: 28.0,
                  decoration: BoxDecoration(
                    border: Border.all(color: Theme.of(context).dividerColor),
                    borderRadius: BorderRadius.circular(8),
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
