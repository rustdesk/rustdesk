import 'dart:async';
import 'dart:io';
import 'dart:convert';

import 'package:auto_size_text/auto_size_text.dart';
import 'package:flutter/material.dart' hide MenuItem;
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/connection_page.dart';
import 'package:flutter_hbb/desktop/pages/desktop_setting_page.dart';
import 'package:flutter_hbb/desktop/pages/desktop_tab_page.dart';
import 'package:flutter_hbb/desktop/widgets/scroll_wrapper.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_hbb/utils/tray_manager.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:window_size/window_size.dart' as window_size;

import '../widgets/button.dart';

class DesktopHomePage extends StatefulWidget {
  const DesktopHomePage({Key? key}) : super(key: key);

  @override
  State<DesktopHomePage> createState() => _DesktopHomePageState();
}

const borderColor = Color(0xFF2F65BA);

class _DesktopHomePageState extends State<DesktopHomePage>
    with TrayListener, AutomaticKeepAliveClientMixin {
  final _leftPaneScrollController = ScrollController();

  @override
  bool get wantKeepAlive => true;
  var updateUrl = '';
  var systemError = '';
  StreamSubscription? _uniLinksSubscription;
  var svcStopped = false.obs;
  var watchIsCanScreenRecording = false;
  var watchIsProcessTrust = false;
  Timer? _updateTimer;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        buildLeftPane(context),
        const VerticalDivider(
          width: 1,
          thickness: 1,
        ),
        Expanded(
          child: buildRightPane(context),
        ),
      ],
    );
  }

  Widget buildLeftPane(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: gFFI.serverModel,
      child: Container(
        width: 200,
        color: Theme.of(context).backgroundColor,
        child: DesktopScrollWrapper(
          scrollController: _leftPaneScrollController,
          child: SingleChildScrollView(
            controller: _leftPaneScrollController,
            child: Column(
              children: [
                buildTip(context),
                buildIDBoard(context),
                buildPasswordBoard(context),
                buildHelpCards(),
              ],
            ),
          ),
        ),
      ),
    );
  }

  buildRightPane(BuildContext context) {
    return Container(
      color: Theme.of(context).scaffoldBackgroundColor,
      child: ConnectionPage(),
    );
  }

  buildIDBoard(BuildContext context) {
    final model = gFFI.serverModel;
    return Container(
      margin: const EdgeInsets.only(left: 20, right: 11),
      height: 57,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.baseline,
        textBaseline: TextBaseline.alphabetic,
        children: [
          Container(
            width: 2,
            decoration: const BoxDecoration(color: MyTheme.accent),
          ).marginOnly(top: 5),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.only(left: 7),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Container(
                    height: 25,
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          translate("ID"),
                          style: TextStyle(
                              fontSize: 14,
                              color: Theme.of(context)
                                  .textTheme
                                  .titleLarge
                                  ?.color
                                  ?.withOpacity(0.5)),
                        ).marginOnly(top: 5),
                        buildPopupMenu(context)
                      ],
                    ),
                  ),
                  Flexible(
                    child: GestureDetector(
                      onDoubleTap: () {
                        Clipboard.setData(
                            ClipboardData(text: model.serverId.text));
                        showToast(translate("Copied"));
                      },
                      child: TextFormField(
                        controller: model.serverId,
                        readOnly: true,
                        decoration: InputDecoration(
                          border: InputBorder.none,
                          contentPadding: EdgeInsets.only(bottom: 20),
                        ),
                        style: TextStyle(
                          fontSize: 22,
                        ),
                      ),
                    ),
                  )
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget buildPopupMenu(BuildContext context) {
    final textColor = Theme.of(context).textTheme.titleLarge?.color;
    RxBool hover = false.obs;
    return InkWell(
      onTap: DesktopTabPage.onAddSetting,
      child: Obx(
        () => CircleAvatar(
          radius: 15,
          backgroundColor: hover.value
              ? Theme.of(context).scaffoldBackgroundColor
              : Theme.of(context).backgroundColor,
          child: Icon(
            Icons.more_vert_outlined,
            size: 20,
            color: hover.value ? textColor : textColor?.withOpacity(0.5),
          ),
        ),
      ),
      onHover: (value) => hover.value = value,
    );
  }

  buildPasswordBoard(BuildContext context) {
    final model = gFFI.serverModel;
    RxBool refreshHover = false.obs;
    RxBool editHover = false.obs;
    final textColor = Theme.of(context).textTheme.titleLarge?.color;
    return Container(
      margin: EdgeInsets.only(left: 20.0, right: 16, top: 13, bottom: 13),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.baseline,
        textBaseline: TextBaseline.alphabetic,
        children: [
          Container(
            width: 2,
            height: 52,
            decoration: BoxDecoration(color: MyTheme.accent),
          ),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.only(left: 7),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  AutoSizeText(
                    translate("One-time Password"),
                    style: TextStyle(
                        fontSize: 14, color: textColor?.withOpacity(0.5)),
                    maxLines: 1,
                  ),
                  Row(
                    children: [
                      Expanded(
                        child: GestureDetector(
                          onDoubleTap: () {
                            if (model.verificationMethod !=
                                kUsePermanentPassword) {
                              Clipboard.setData(
                                  ClipboardData(text: model.serverPasswd.text));
                              showToast(translate("Copied"));
                            }
                          },
                          child: TextFormField(
                            controller: model.serverPasswd,
                            readOnly: true,
                            decoration: InputDecoration(
                              border: InputBorder.none,
                              contentPadding: EdgeInsets.only(bottom: 2),
                            ),
                            style: TextStyle(fontSize: 15),
                          ),
                        ),
                      ),
                      InkWell(
                        child: Obx(
                          () => Icon(
                            Icons.refresh,
                            color: refreshHover.value
                                ? textColor
                                : Color(0xFFDDDDDD), // TODO
                            size: 22,
                          ).marginOnly(right: 8, bottom: 2),
                        ),
                        onTap: () => bind.mainUpdateTemporaryPassword(),
                        onHover: (value) => refreshHover.value = value,
                      ),
                      InkWell(
                        child: Obx(
                          () => Icon(
                            Icons.edit,
                            color: editHover.value
                                ? textColor
                                : Color(0xFFDDDDDD), // TODO
                            size: 22,
                          ).marginOnly(right: 8, bottom: 2),
                        ),
                        onTap: () => DesktopSettingPage.switch2page(1),
                        onHover: (value) => editHover.value = value,
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  buildTip(BuildContext context) {
    return Padding(
      padding:
          const EdgeInsets.only(left: 20.0, right: 16, top: 16.0, bottom: 5),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.start,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            translate("Your Desktop"),
            style: Theme.of(context).textTheme.titleLarge,
            // style: TextStyle(
            //     // color: MyTheme.color(context).text,
            //     fontWeight: FontWeight.normal,
            //     fontSize: 19),
          ),
          SizedBox(
            height: 10.0,
          ),
          Text(
            translate("desk_tip"),
            overflow: TextOverflow.clip,
            style: Theme.of(context).textTheme.bodySmall,
          )
        ],
      ),
    );
  }

  Widget buildHelpCards() {
    if (Platform.isWindows) {
      if (!bind.mainIsInstalled()) {
        return buildInstallCard(
            "", "install_tip", "Install", bind.mainGotoInstall);
      } else if (bind.mainIsInstalledLowerVersion()) {
        return buildInstallCard("Status", "Your installation is lower version.",
            "Click to upgrade", bind.mainUpdateMe);
      }
    }
    if (updateUrl.isNotEmpty) {
      return buildInstallCard(
          "Status",
          "There is a newer version of ${bind.mainGetAppNameSync()} ${bind.mainGetNewVersion()} available.",
          "Click to download", () async {
        final Uri url = Uri.parse('https://rustdesk.com');
        await launchUrl(url);
      });
    }
    if (systemError.isNotEmpty) {
      return buildInstallCard("", systemError, "", () {});
    }
    if (Platform.isMacOS) {
      if (!bind.mainIsCanScreenRecording(prompt: false)) {
        return buildInstallCard("Permissions", "config_screen", "Configure",
            () async {
          bind.mainIsCanScreenRecording(prompt: true);
          watchIsCanScreenRecording = true;
        }, help: 'Help', link: translate("doc_mac_permission"));
      } else if (!bind.mainIsProcessTrusted(prompt: false)) {
        return buildInstallCard("Permissions", "config_acc", "Configure",
            () async {
          bind.mainIsProcessTrusted(prompt: true);
          watchIsProcessTrust = true;
        }, help: 'Help', link: translate("doc_mac_permission"));
      } else if (!svcStopped.value &&
          bind.mainIsInstalled() &&
          !bind.mainIsInstalledDaemon(prompt: false)) {
        return buildInstallCard("", "install_daemon_tip", "Install", () async {
          bind.mainIsInstalledDaemon(prompt: true);
        });
      }
    }
    if (bind.mainIsInstalledLowerVersion()) {}
    return Container();
  }

  Widget buildInstallCard(String title, String content, String btnText,
      GestureTapCallback onPressed,
      {String? help, String? link}) {
    return Container(
      margin: EdgeInsets.only(top: 20),
      child: Container(
          decoration: BoxDecoration(
              gradient: LinearGradient(
            begin: Alignment.centerLeft,
            end: Alignment.centerRight,
            colors: [
              Color.fromARGB(255, 226, 66, 188),
              Color.fromARGB(255, 244, 114, 124),
            ],
          )),
          padding: EdgeInsets.all(20),
          child: Column(
              mainAxisAlignment: MainAxisAlignment.start,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: (title.isNotEmpty
                      ? <Widget>[
                          Center(
                              child: Text(
                            translate(title),
                            style: TextStyle(
                                color: Colors.white,
                                fontWeight: FontWeight.bold,
                                fontSize: 15),
                          ).marginOnly(bottom: 6)),
                        ]
                      : <Widget>[]) +
                  <Widget>[
                    Text(
                      translate(content),
                      style: TextStyle(
                          height: 1.5,
                          color: Colors.white,
                          fontWeight: FontWeight.normal,
                          fontSize: 13),
                    ).marginOnly(bottom: 20)
                  ] +
                  (btnText.isNotEmpty
                      ? <Widget>[
                          Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                FixedWidthButton(
                                  width: 150,
                                  padding: 8,
                                  isOutline: true,
                                  text: translate(btnText),
                                  textColor: Colors.white,
                                  borderColor: Colors.white,
                                  textSize: 20,
                                  radius: 10,
                                  onTap: onPressed,
                                )
                              ])
                        ]
                      : <Widget>[]) +
                  (help != null
                      ? <Widget>[
                          Center(
                              child: InkWell(
                                  onTap: () async =>
                                      await launchUrl(Uri.parse(link!)),
                                  child: Text(
                                    translate(help),
                                    style: TextStyle(
                                        decoration: TextDecoration.underline,
                                        color: Colors.white,
                                        fontSize: 12),
                                  )).marginOnly(top: 6)),
                        ]
                      : <Widget>[]))),
    );
  }

  @override
  void onTrayIconMouseDown() {
    windowManager.show();
  }

  @override
  void onTrayIconRightMouseDown() {
    // linux does not support popup menu manually.
    // linux will handle popup action ifself.
    if (Platform.isMacOS || Platform.isWindows) {
      trayManager.popUpContextMenu();
    }
  }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) {
    switch (menuItem.key) {
      case kTrayItemQuitKey:
        windowManager.close();
        break;
      case kTrayItemShowKey:
        windowManager.show();
        windowManager.focus();
        break;
      default:
        break;
    }
  }

  @override
  void initState() {
    super.initState();
    bind.mainStartGrabKeyboard();
    _updateTimer = periodic_immediate(const Duration(seconds: 1), () async {
      await gFFI.serverModel.fetchID();
      final url = await bind.mainGetSoftwareUpdateUrl();
      if (updateUrl != url) {
        updateUrl = url;
        setState(() {});
      }
      final error = await bind.mainGetError();
      if (systemError != error) {
        systemError = error;
        setState(() {});
      }
      final v = await bind.mainGetOption(key: "stop-service") == "Y";
      if (v != svcStopped.value) {
        svcStopped.value = v;
        setState(() {});
      }
      if (watchIsCanScreenRecording) {
        if (bind.mainIsCanScreenRecording(prompt: false)) {
          watchIsCanScreenRecording = false;
          setState(() {});
        }
      }
      if (watchIsProcessTrust) {
        if (bind.mainIsProcessTrusted(prompt: false)) {
          watchIsProcessTrust = false;
          setState(() {});
        }
      }
    });
    Get.put<RxBool>(svcStopped, tag: 'stop-service');
    // disable this tray because we use tray function provided by rust now
    // initTray();
    trayManager.addListener(this);
    rustDeskWinManager.registerActiveWindowListener(onActiveWindowChanged);
    // main window may be hidden because of the initial uni link or arguments.
    // note that we must wrap this active window registration in future because
    // we must ensure the execution is after `windowManager.hide/show()`.
    Future.delayed(Duration.zero, () {
      windowManager.isVisible().then((visibility) {
        if (visibility) {
          rustDeskWinManager.registerActiveWindow(kWindowMainId);
        }
      });
    });

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      debugPrint(
          "[Main] call ${call.method} with args ${call.arguments} from window $fromWindowId");
      if (call.method == "main_window_on_top") {
        window_on_top(null);
      } else if (call.method == "get_window_info") {
        final screen = (await window_size.getWindowInfo()).screen;
        if (screen == null) {
          return "";
        } else {
          return jsonEncode({
            'frame': {
              'l': screen.frame.left,
              't': screen.frame.top,
              'r': screen.frame.right,
              'b': screen.frame.bottom,
            },
            'visibleFrame': {
              'l': screen.visibleFrame.left,
              't': screen.visibleFrame.top,
              'r': screen.visibleFrame.right,
              'b': screen.visibleFrame.bottom,
            },
            'scaleFactor': screen.scaleFactor,
          });
        }
      } else if (call.method == kWindowActionRebuild) {
        reloadCurrentWindow();
      } else if (call.method == kWindowEventShow) {
        rustDeskWinManager.registerActiveWindow(call.arguments["id"]);
      } else if (call.method == kWindowEventHide) {
        rustDeskWinManager.unregisterActiveWindow(call.arguments["id"]);
      }
    });
    _uniLinksSubscription = listenUniLinks();
  }

  @override
  void dispose() {
    // destoryTray();
    // fix: disable unregister to prevent from receiving events from other windows
    // rustDeskWinManager.unregisterActiveWindowListener(onActiveWindowChanged);
    trayManager.removeListener(this);
    _uniLinksSubscription?.cancel();
    Get.delete<RxBool>(tag: 'stop-service');
    _updateTimer?.cancel();
    super.dispose();
  }
}

void setPasswordDialog() async {
  final pw = await bind.mainGetPermanentPassword();
  final p0 = TextEditingController(text: pw);
  final p1 = TextEditingController(text: pw);
  var errMsg0 = "";
  var errMsg1 = "";

  gFFI.dialogManager.show((setState, close) {
    submit() {
      setState(() {
        errMsg0 = "";
        errMsg1 = "";
      });
      final pass = p0.text.trim();
      if (pass.length < 6 && pass.isNotEmpty) {
        setState(() {
          errMsg0 = translate("Too short, at least 6 characters.");
        });
        return;
      }
      if (p1.text.trim() != pass) {
        setState(() {
          errMsg1 = translate("The confirmation is not identical.");
        });
        return;
      }
      bind.mainSetPermanentPassword(password: pass);
      close();
    }

    return CustomAlertDialog(
      title: Text(translate("Set Password")),
      content: ConstrainedBox(
        constraints: const BoxConstraints(minWidth: 500),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: const BoxConstraints(minWidth: 100),
                    child: Text(
                      "${translate('Password')}:",
                      textAlign: TextAlign.start,
                    ).marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    obscureText: true,
                    decoration: InputDecoration(
                        border: const OutlineInputBorder(),
                        errorText: errMsg0.isNotEmpty ? errMsg0 : null),
                    controller: p0,
                    focusNode: FocusNode()..requestFocus(),
                  ),
                ),
              ],
            ),
            const SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: const BoxConstraints(minWidth: 100),
                    child: Text("${translate('Confirmation')}:")
                        .marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    obscureText: true,
                    decoration: InputDecoration(
                        border: const OutlineInputBorder(),
                        errorText: errMsg1.isNotEmpty ? errMsg1 : null),
                    controller: p1,
                  ),
                ),
              ],
            ),
          ],
        ),
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
