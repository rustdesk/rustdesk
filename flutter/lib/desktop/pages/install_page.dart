import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher_string.dart';
import 'package:window_manager/window_manager.dart';

class InstallPage extends StatefulWidget {
  const InstallPage({Key? key}) : super(key: key);

  @override
  State<InstallPage> createState() => _InstallPageState();
}

class _InstallPageState extends State<InstallPage> {
  final tabController = DesktopTabController(tabType: DesktopTabType.main);

  @override
  void initState() {
    super.initState();
    Get.put<DesktopTabController>(tabController);
    const lable = "install";
    tabController.add(TabInfo(
        key: lable,
        label: lable,
        closable: false,
        page: _InstallPageBody(
          key: const ValueKey(lable),
        )));
  }

  @override
  void dispose() {
    super.dispose();
    Get.delete<DesktopTabController>();
  }

  @override
  Widget build(BuildContext context) {
    return DragToResizeArea(
      resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
      child: Container(
        child: Scaffold(
            backgroundColor: Theme.of(context).colorScheme.background,
            body: DesktopTab(controller: tabController)),
      ),
    );
  }
}

class _InstallPageBody extends StatefulWidget {
  const _InstallPageBody({Key? key}) : super(key: key);

  @override
  State<_InstallPageBody> createState() => _InstallPageBodyState();
}

class _InstallPageBodyState extends State<_InstallPageBody>
    with WindowListener {
  late final TextEditingController controller;
  final RxBool startmenu = true.obs;
  final RxBool desktopicon = true.obs;
  final RxBool driverCert = true.obs;
  final RxBool showProgress = false.obs;
  final RxBool btnEnabled = true.obs;

  @override
  void initState() {
    windowManager.addListener(this);
    controller = TextEditingController(text: bind.installInstallPath());
    super.initState();
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    super.dispose();
  }

  @override
  void onWindowClose() {
    gFFI.close();
    super.onWindowClose();
    windowManager.setPreventClose(false);
    windowManager.close();
  }

  @override
  Widget build(BuildContext context) {
    final double em = 13;
    final btnFontSize = 0.9 * em;
    final double button_radius = 6;
    final isDarkTheme = MyTheme.currentThemeMode() == ThemeMode.dark;
    final buttonStyle = OutlinedButton.styleFrom(
        shape: RoundedRectangleBorder(
      borderRadius: BorderRadius.all(Radius.circular(button_radius)),
    ));
    final inputBorder = OutlineInputBorder(
        borderRadius: BorderRadius.zero,
        borderSide:
            BorderSide(color: isDarkTheme ? Colors.white70 : Colors.black12));
    final textColor = isDarkTheme ? null : Colors.black87;
    final dividerColor = isDarkTheme ? Colors.white70 : Colors.black87;
    return Scaffold(
        backgroundColor: null,
        body: SingleChildScrollView(
          child: Column(
            children: [
              Row(
                children: [
                  Text(
                    translate('Installation'),
                    style: TextStyle(
                        fontSize: 2 * em, fontWeight: FontWeight.w500),
                  ),
                ],
              ),
              Row(
                children: [
                  Text('${translate('Installation Path')}: '),
                  Expanded(
                      child: TextField(
                    controller: controller,
                    readOnly: true,
                    style: TextStyle(
                        fontSize: 1.5 * em, fontWeight: FontWeight.w400),
                    decoration: InputDecoration(
                      isDense: true,
                      contentPadding: EdgeInsets.all(0.75 * em),
                      enabledBorder: inputBorder,
                      border: inputBorder,
                      focusedBorder: inputBorder,
                      constraints: BoxConstraints(maxHeight: 3 * em),
                    ),
                  )),
                  Obx(() => OutlinedButton(
                          onPressed:
                              btnEnabled.value ? selectInstallPath : null,
                          style: buttonStyle,
                          child: Text(translate('Change Path'),
                              style: TextStyle(
                                  color: textColor, fontSize: btnFontSize)))
                      .marginOnly(left: em))
                ],
              ).marginSymmetric(vertical: 2 * em),
              Row(
                children: [
                  Obx(() => Checkbox(
                      value: startmenu.value,
                      onChanged: (b) {
                        if (b != null) startmenu.value = b;
                      })),
                  Text(translate('Create start menu shortcuts'))
                ],
              ),
              Row(
                children: [
                  Obx(() => Checkbox(
                      value: desktopicon.value,
                      onChanged: (b) {
                        if (b != null) desktopicon.value = b;
                      })),
                  Text(translate('Create desktop icon'))
                ],
              ),
              Offstage(
                offstage: !Platform.isWindows,
                child: Row(
                  children: [
                    Obx(() => Checkbox(
                        value: driverCert.value,
                        onChanged: (b) {
                          if (b != null) driverCert.value = b;
                        })),
                    Text(
                        '${translate('Install driver cert (test cert)')} ${translate('Virtual display need')}')
                  ],
                ),
              ),
              GestureDetector(
                  onTap: () => launchUrlString('http://rustdesk.com/privacy'),
                  child: Row(
                    children: [
                      Text(translate('End-user license agreement'),
                          style: const TextStyle(
                              decoration: TextDecoration.underline))
                    ],
                  )).marginOnly(top: 2 * em),
              Row(children: [Text(translate('agreement_tip'))])
                  .marginOnly(top: em),
              Divider(color: dividerColor).marginSymmetric(vertical: 0.5 * em),
              Row(
                children: [
                  Expanded(
                      child: Obx(() => Offstage(
                            offstage: !showProgress.value,
                            child: LinearProgressIndicator(),
                          ))),
                  Obx(() => OutlinedButton(
                          onPressed: btnEnabled.value
                              ? () => windowManager.close()
                              : null,
                          style: buttonStyle,
                          child: Text(translate('Cancel'),
                              style: TextStyle(
                                  color: textColor, fontSize: btnFontSize)))
                      .marginOnly(right: 2 * em)),
                  Obx(() => ElevatedButton(
                      onPressed: btnEnabled.value ? install : null,
                      style: ElevatedButton.styleFrom(
                          primary: MyTheme.button,
                          shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.all(
                                Radius.circular(button_radius)),
                          )),
                      child: Text(
                        translate('Accept and Install'),
                        style: TextStyle(fontSize: btnFontSize),
                      ))),
                  Offstage(
                    offstage: bind.installShowRunWithoutInstall(),
                    child: Obx(() => OutlinedButton(
                            onPressed: btnEnabled.value
                                ? () => bind.installRunWithoutInstall()
                                : null,
                            style: buttonStyle,
                            child: Text(translate('Run without install'),
                                style: TextStyle(
                                    color: textColor, fontSize: btnFontSize)))
                        .marginOnly(left: 2 * em)),
                  ),
                ],
              )
            ],
          ).paddingSymmetric(horizontal: 8 * em, vertical: 2 * em),
        ));
  }

  void install() {
    do_install() {
      btnEnabled.value = false;
      showProgress.value = true;
      String args = '';
      if (startmenu.value) args += ' startmenu';
      if (desktopicon.value) args += ' desktopicon';
      if (driverCert.value) args += ' driverCert';
      bind.installInstallMe(options: args, path: controller.text);
    }

    if (driverCert.isTrue) {
      final tag = 'install-info-install-cert-confirm';
      final btns = [
        dialogButton(
          'Cancel',
          onPressed: () => gFFI.dialogManager.dismissByTag(tag),
          isOutline: true,
        ),
        dialogButton(
          'OK',
          onPressed: () {
            gFFI.dialogManager.dismissByTag(tag);
            do_install();
          },
          isOutline: false,
        ),
      ];
      gFFI.dialogManager.show(
        (setState, close) => CustomAlertDialog(
          title: null,
          content: SelectionArea(
              child: msgboxContent('info', '', 'instsall_cert_tip')),
          actions: btns,
          onCancel: close,
        ),
        tag: tag,
      );
    } else {
      do_install();
    }
  }

  void selectInstallPath() async {
    String? install_path = await FilePicker.platform
        .getDirectoryPath(initialDirectory: controller.text);
    if (install_path != null) {
      install_path = '$install_path\\${await bind.mainGetAppName()}';
      controller.text = install_path;
    }
  }
}
