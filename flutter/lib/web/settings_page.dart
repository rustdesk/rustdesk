import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/pages/desktop_setting_page.dart';
import 'package:flutter_hbb/mobile/pages/scan_page.dart';
import 'package:flutter_hbb/mobile/pages/settings_page.dart';
import 'package:provider/provider.dart';

import '../../common.dart';
import '../../common/widgets/login.dart';
import '../../models/model.dart';

class WebSettingsPage extends StatelessWidget {
  const WebSettingsPage({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    if (isWebDesktop) {
      return _buildDesktopButton(context);
    } else {
      return _buildMobileMenu(context);
    }
  }

  Widget _buildDesktopButton(BuildContext context) {
    return IconButton(
      icon: const Icon(Icons.more_vert),
      onPressed: () {
        Navigator.push(
          context,
          MaterialPageRoute(
            builder: (BuildContext context) =>
                DesktopSettingPage(initialTabkey: SettingsTabKey.general),
          ),
        );
      },
    );
  }

  Widget _buildMobileMenu(BuildContext context) {
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
                  child: Text(translate('About RustDesk')),
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

