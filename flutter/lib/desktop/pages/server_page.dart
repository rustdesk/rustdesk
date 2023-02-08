// original cm window in Sciter version.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../common.dart';
import '../../common/widgets/chat_page.dart';
import '../../models/platform_model.dart';
import '../../models/server_model.dart';

class DesktopServerPage extends StatefulWidget {
  const DesktopServerPage({Key? key}) : super(key: key);

  @override
  State<DesktopServerPage> createState() => _DesktopServerPageState();
}

class _DesktopServerPageState extends State<DesktopServerPage>
    with WindowListener, AutomaticKeepAliveClientMixin {
  final tabController = gFFI.serverModel.tabController;
  @override
  void initState() {
    gFFI.ffiModel.updateEventListener("");
    windowManager.addListener(this);
    tabController.onRemoved = (_, id) {
      onRemoveId(id);
    };
    tabController.onSelected = (_, id) {
      windowManager.setTitle(getWindowNameWithId(id));
    };
    super.initState();
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    super.dispose();
  }

  @override
  void onWindowClose() {
    gFFI.serverModel.closeAll();
    gFFI.close();
    super.onWindowClose();
  }

  void onRemoveId(String id) {
    if (tabController.state.value.tabs.isEmpty) {
      windowManager.close();
    }
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return MultiProvider(
        providers: [
          ChangeNotifierProvider.value(value: gFFI.serverModel),
          ChangeNotifierProvider.value(value: gFFI.chatModel),
        ],
        child: Consumer<ServerModel>(
            builder: (context, serverModel, child) => Container(
                decoration: BoxDecoration(
                    border: Border.all(color: MyTheme.color(context).border!)),
                child: Scaffold(
                  backgroundColor: Theme.of(context).backgroundColor,
                  body: Center(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.start,
                      children: [
                        Expanded(child: ConnectionManager()),
                      ],
                    ),
                  ),
                ))));
  }

  @override
  bool get wantKeepAlive => true;
}

class ConnectionManager extends StatefulWidget {
  @override
  State<StatefulWidget> createState() => ConnectionManagerState();
}

class ConnectionManagerState extends State<ConnectionManager> {
  @override
  void initState() {
    gFFI.serverModel.updateClientState();
    gFFI.serverModel.tabController.onSelected = (index, _) =>
        gFFI.chatModel.changeCurrentID(gFFI.serverModel.clients[index].id);
    gFFI.chatModel.isConnManager = true;
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final serverModel = Provider.of<ServerModel>(context);
    pointerHandler(PointerEvent e) {
      if (serverModel.cmHiddenTimer != null) {
        serverModel.cmHiddenTimer!.cancel();
        serverModel.cmHiddenTimer = null;
        debugPrint("CM hidden timer has been canceled");
      }
    }

    return serverModel.clients.isEmpty
        ? Column(
            children: [
              buildTitleBar(),
              Expanded(
                child: Center(
                  child: Text(translate("Waiting")),
                ),
              ),
            ],
          )
        : Listener(
            onPointerDown: pointerHandler,
            onPointerMove: pointerHandler,
            child: DesktopTab(
                showTitle: false,
                showMaximize: false,
                showMinimize: true,
                showClose: true,
                onWindowCloseButton: handleWindowCloseButton,
                controller: serverModel.tabController,
                maxLabelWidth: 100,
                tail: buildScrollJumper(),
                selectedTabBackgroundColor:
                    Theme.of(context).hintColor.withOpacity(0.2),
                tabBuilder: (key, icon, label, themeConf) {
                  final client = serverModel.clients.firstWhereOrNull(
                      (client) => client.id.toString() == key);
                  return Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Tooltip(
                          message: key,
                          waitDuration: Duration(seconds: 1),
                          child: label),
                      Obx(() => Offstage(
                          offstage:
                              !(client?.hasUnreadChatMessage.value ?? false),
                          child:
                              Icon(Icons.circle, color: Colors.red, size: 10)))
                    ],
                  );
                },
                pageViewBuilder: (pageView) => Row(children: [
                      Expanded(child: pageView),
                      Consumer<ChatModel>(
                          builder: (_, model, child) => model.isShowCMChatPage
                              ? Expanded(child: Scaffold(body: ChatPage()))
                              : Offstage())
                    ])));
  }

  Widget buildTitleBar() {
    return SizedBox(
      height: kDesktopRemoteTabBarHeight,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const _AppIcon(),
          Expanded(
            child: GestureDetector(
              onPanStart: (d) {
                windowManager.startDragging();
              },
              child: Container(
                color: Theme.of(context).backgroundColor,
              ),
            ),
          ),
          const SizedBox(
            width: 4.0,
          ),
          const _CloseButton()
        ],
      ),
    );
  }

  Widget buildScrollJumper() {
    final offstage = gFFI.serverModel.clients.length < 2;
    final sc = gFFI.serverModel.tabController.state.value.scrollController;
    return Offstage(
        offstage: offstage,
        child: Row(
          children: [
            ActionIcon(
                icon: Icons.arrow_left, iconSize: 22, onTap: sc.backward),
            ActionIcon(
                icon: Icons.arrow_right, iconSize: 22, onTap: sc.forward),
          ],
        ));
  }

  Future<bool> handleWindowCloseButton() async {
    var tabController = gFFI.serverModel.tabController;
    final connLength = tabController.length;
    if (connLength <= 1) {
      windowManager.close();
      return true;
    } else {
      final opt = "enable-confirm-closing-tabs";
      final bool res;
      if (!option2bool(opt, await bind.mainGetOption(key: opt))) {
        res = true;
      } else {
        res = await closeConfirmDialog();
      }
      if (res) {
        windowManager.close();
      }
      return res;
    }
  }
}

Widget buildConnectionCard(Client client) {
  return Consumer<ServerModel>(
      builder: (context, value, child) => Column(
            mainAxisAlignment: MainAxisAlignment.start,
            crossAxisAlignment: CrossAxisAlignment.start,
            key: ValueKey(client.id),
            children: [
              _CmHeader(client: client),
              client.type_() != ClientType.remote || client.disconnected
                  ? Offstage()
                  : _PrivilegeBoard(client: client),
              Expanded(
                  child: Align(
                alignment: Alignment.bottomCenter,
                child: _CmControlPanel(client: client),
              ))
            ],
          ).paddingSymmetric(vertical: 8.0, horizontal: 8.0));
}

class _AppIcon extends StatelessWidget {
  const _AppIcon({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: EdgeInsets.symmetric(horizontal: 4.0),
      child: SvgPicture.asset(
        'assets/logo.svg',
        width: 30,
        height: 30,
      ),
    );
  }
}

class _CloseButton extends StatelessWidget {
  const _CloseButton({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return IconButton(
      onPressed: () {
        windowManager.close();
      },
      icon: const Icon(
        IconFont.close,
        size: 18,
      ),
      splashColor: Colors.transparent,
      hoverColor: Colors.transparent,
    );
  }
}

class _CmHeader extends StatefulWidget {
  final Client client;

  const _CmHeader({Key? key, required this.client}) : super(key: key);

  @override
  State<_CmHeader> createState() => _CmHeaderState();
}

class _CmHeaderState extends State<_CmHeader>
    with AutomaticKeepAliveClientMixin {
  Client get client => widget.client;

  final _time = 0.obs;
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _timer = Timer.periodic(Duration(seconds: 1), (_) {
      if (client.authorized && !client.disconnected) {
        _time.value = _time.value + 1;
      }
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        // icon
        Container(
          width: 90,
          height: 90,
          alignment: Alignment.center,
          decoration: BoxDecoration(color: str2color(client.name)),
          child: Text(
            client.name[0],
            style: TextStyle(
                fontWeight: FontWeight.bold, color: Colors.white, fontSize: 65),
          ),
        ).marginOnly(left: 4.0, right: 8.0),
        Expanded(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.start,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              FittedBox(
                  child: Text(
                client.name,
                style: TextStyle(
                  color: MyTheme.cmIdColor,
                  fontWeight: FontWeight.bold,
                  fontSize: 20,
                  overflow: TextOverflow.ellipsis,
                ),
                maxLines: 1,
              )),
              FittedBox(
                  child: Text("(${client.peerId})",
                      style:
                          TextStyle(color: MyTheme.cmIdColor, fontSize: 14))),
              SizedBox(
                height: 16.0,
              ),
              FittedBox(
                  child: Row(
                children: [
                  Text(client.authorized
                          ? client.disconnected
                              ? translate("Disconnected")
                              : translate("Connected")
                          : "${translate("Request access to your device")}...")
                      .marginOnly(right: 8.0),
                  if (client.authorized)
                    Obx(() => Text(
                        formatDurationToTime(Duration(seconds: _time.value))))
                ],
              ))
            ],
          ),
        ),
        Offstage(
          offstage: !client.authorized || client.type_() != ClientType.remote,
          child: IconButton(
              onPressed: () => checkClickTime(
                  client.id, () => gFFI.chatModel.toggleCMChatPage(client.id)),
              icon: Icon(Icons.message_outlined),
              splashRadius: kDesktopIconButtonSplashRadius),
        )
      ],
    );
  }

  @override
  bool get wantKeepAlive => true;
}

class _PrivilegeBoard extends StatefulWidget {
  final Client client;

  const _PrivilegeBoard({Key? key, required this.client}) : super(key: key);

  @override
  State<StatefulWidget> createState() => _PrivilegeBoardState();
}

class _PrivilegeBoardState extends State<_PrivilegeBoard> {
  late final client = widget.client;
  Widget buildPermissionIcon(
      bool enabled, ImageProvider icon, Function(bool)? onTap, String tooltip) {
    return Tooltip(
      message: tooltip,
      child: Ink(
        decoration:
            BoxDecoration(color: enabled ? MyTheme.accent80 : Colors.grey),
        padding: EdgeInsets.all(4.0),
        child: InkWell(
          onTap: () =>
              checkClickTime(widget.client.id, () => onTap?.call(!enabled)),
          child: Image(
            image: icon,
            width: 50,
            height: 50,
            fit: BoxFit.scaleDown,
          ),
        ),
      ).marginSymmetric(horizontal: 4.0),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: EdgeInsets.only(top: 16.0, bottom: 8.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            translate("Permissions"),
            style: TextStyle(fontSize: 16),
          ).marginOnly(left: 4.0),
          SizedBox(
            height: 8.0,
          ),
          FittedBox(
              child: Row(
            children: [
              buildPermissionIcon(client.keyboard, iconKeyboard, (enabled) {
                bind.cmSwitchPermission(
                    connId: client.id, name: "keyboard", enabled: enabled);
                setState(() {
                  client.keyboard = enabled;
                });
              }, translate('Allow using keyboard and mouse')),
              buildPermissionIcon(client.clipboard, iconClipboard, (enabled) {
                bind.cmSwitchPermission(
                    connId: client.id, name: "clipboard", enabled: enabled);
                setState(() {
                  client.clipboard = enabled;
                });
              }, translate('Allow using clipboard')),
              buildPermissionIcon(client.audio, iconAudio, (enabled) {
                bind.cmSwitchPermission(
                    connId: client.id, name: "audio", enabled: enabled);
                setState(() {
                  client.audio = enabled;
                });
              }, translate('Allow hearing sound')),
              buildPermissionIcon(client.file, iconFile, (enabled) {
                bind.cmSwitchPermission(
                    connId: client.id, name: "file", enabled: enabled);
                setState(() {
                  client.file = enabled;
                });
              }, translate('Allow file copy and paste')),
              buildPermissionIcon(client.restart, iconRestart, (enabled) {
                bind.cmSwitchPermission(
                    connId: client.id, name: "restart", enabled: enabled);
                setState(() {
                  client.restart = enabled;
                });
              }, translate('Allow remote restart')),
              buildPermissionIcon(client.recording, iconRecording, (enabled) {
                bind.cmSwitchPermission(
                    connId: client.id, name: "recording", enabled: enabled);
                setState(() {
                  client.recording = enabled;
                });
              }, translate('Allow recording session'))
            ],
          )),
        ],
      ),
    );
  }
}

const double bigMargin = 15;

class _CmControlPanel extends StatelessWidget {
  final Client client;

  const _CmControlPanel({Key? key, required this.client}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return client.authorized
        ? client.disconnected
            ? buildDisconnected(context)
            : buildAuthorized(context)
        : buildUnAuthorized(context);
  }

  buildAuthorized(BuildContext context) {
    final bool canElevate = bind.cmCanElevate();
    final model = Provider.of<ServerModel>(context);
    final showElevation = canElevate &&
        model.showElevation &&
        client.type_() == ClientType.remote;
    return Column(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        Offstage(
          offstage: !client.inVoiceCall,
          child: buildButton(context,
              color: Colors.red,
              onClick: () => closeVoiceCall(),
              icon: Icon(Icons.phone_disabled_rounded, color: Colors.white),
              text: "Stop voice call",
              textColor: Colors.white),
        ),
        Offstage(
          offstage: !client.incomingVoiceCall,
          child: Row(
            children: [
              Expanded(
                child: buildButton(context,
                    color: MyTheme.accent,
                    onClick: () => handleVoiceCall(true),
                    icon: Icon(Icons.phone_enabled, color: Colors.white),
                    text: "Accept",
                    textColor: Colors.white),
              ),
              Expanded(
                child: buildButton(context,
                    color: Colors.red,
                    onClick: () => handleVoiceCall(false),
                    icon:
                        Icon(Icons.phone_disabled_rounded, color: Colors.white),
                    text: "Dismiss",
                    textColor: Colors.white),
              )
            ],
          ),
        ),
        Offstage(
          offstage: !client.fromSwitch,
          child: buildButton(context,
              color: Colors.purple,
              onClick: () => handleSwitchBack(context),
              icon: Icon(Icons.reply, color: Colors.white),
              text: "Switch Sides",
              textColor: Colors.white),
        ),
        Offstage(
          offstage: !showElevation,
          child: buildButton(context, color: Colors.green[700], onClick: () {
            handleElevate(context);
            windowManager.minimize();
          },
              icon: Icon(
                Icons.security_sharp,
                color: Colors.white,
              ),
              text: 'Elevate',
              textColor: Colors.white),
        ),
        Row(
          children: [
            Expanded(
                child: buildButton(context,
                    color: Colors.redAccent,
                    onClick: handleDisconnect,
                    text: 'Disconnect',
                    textColor: Colors.white)),
          ],
        )
      ],
    )
        .marginOnly(bottom: showElevation ? 0 : bigMargin)
        .marginSymmetric(horizontal: showElevation ? 0 : bigMargin);
  }

  buildDisconnected(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Expanded(
            child: buildButton(context,
                color: MyTheme.accent,
                onClick: handleClose,
                text: 'Close',
                textColor: Colors.white)),
      ],
    ).marginOnly(bottom: 15).marginSymmetric(horizontal: bigMargin);
  }

  buildUnAuthorized(BuildContext context) {
    final bool canElevate = bind.cmCanElevate();
    final model = Provider.of<ServerModel>(context);
    final showElevation = canElevate &&
        model.showElevation &&
        client.type_() == ClientType.remote;
    final showAccept = model.approveMode != 'password';
    return Column(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        Offstage(
          offstage: !showElevation || !showAccept,
          child: buildButton(context, color: Colors.green[700], onClick: () {
            handleAccept(context);
            handleElevate(context);
            windowManager.minimize();
          },
              text: 'Accept',
              icon: Icon(
                Icons.security_sharp,
                color: Colors.white,
              ),
              textColor: Colors.white),
        ),
        Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            if (showAccept)
              Expanded(
                child: Column(
                  children: [
                    buildButton(context, color: MyTheme.accent, onClick: () {
                      handleAccept(context);
                      windowManager.minimize();
                    }, text: 'Accept', textColor: Colors.white),
                  ],
                ),
              ),
            Expanded(
                child: buildButton(context,
                    color: Colors.transparent,
                    border: Border.all(color: Colors.grey),
                    onClick: handleDisconnect,
                    text: 'Cancel',
                    textColor: null)),
          ],
        ),
      ],
    )
        .marginOnly(bottom: showElevation ? 0 : bigMargin)
        .marginSymmetric(horizontal: showElevation ? 0 : bigMargin);
  }

  Widget buildButton(
    BuildContext context, {
    required Color? color,
    required Function() onClick,
    Icon? icon,
    BoxBorder? border,
    required String text,
    required Color? textColor,
  }) {
    Widget textWidget;
    if (icon != null) {
      textWidget = Text(
        translate(text),
        style: TextStyle(color: textColor),
        textAlign: TextAlign.center,
      );
    } else {
      textWidget = Expanded(
        child: Text(
          translate(text),
          style: TextStyle(color: textColor),
          textAlign: TextAlign.center,
        ),
      );
    }
    return Container(
      height: 30,
      decoration: BoxDecoration(
          color: color, borderRadius: BorderRadius.circular(4), border: border),
      child: InkWell(
          onTap: () => checkClickTime(client.id, onClick),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Offstage(offstage: icon == null, child: icon),
              textWidget,
            ],
          )),
    ).marginAll(4);
  }

  void handleDisconnect() {
    bind.cmCloseConnection(connId: client.id);
  }

  void handleAccept(BuildContext context) {
    final model = Provider.of<ServerModel>(context, listen: false);
    model.sendLoginResponse(client, true);
  }

  void handleElevate(BuildContext context) {
    final model = Provider.of<ServerModel>(context, listen: false);
    model.setShowElevation(false);
    bind.cmElevatePortable(connId: client.id);
  }

  void handleClose() async {
    await bind.cmRemoveDisconnectedConnection(connId: client.id);
    if (await bind.cmGetClientsLength() == 0) {
      windowManager.close();
    }
  }

  void handleSwitchBack(BuildContext context) {
    bind.cmSwitchBack(connId: client.id);
  }

  void handleVoiceCall(bool accept) {
    bind.cmHandleIncomingVoiceCall(id: client.id, accept: accept);
  }

  void closeVoiceCall() {
    bind.cmCloseVoiceCall(id: client.id);
  }
}

void checkClickTime(int id, Function() callback) async {
  var clickCallbackTime = DateTime.now().millisecondsSinceEpoch;
  await bind.cmCheckClickTime(connId: id);
  Timer(const Duration(milliseconds: 120), () async {
    var d = clickCallbackTime - await bind.cmGetClickTime();
    if (d > 120) callback();
  });
}
