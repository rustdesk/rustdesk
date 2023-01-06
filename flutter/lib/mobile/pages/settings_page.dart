import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:settings_ui/settings_ui.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../common.dart';
import '../../common/widgets/dialog.dart';
import '../../desktop/widgets/login.dart';
import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../widgets/dialog.dart';
import 'home_page.dart';
import 'scan_page.dart';

class SettingsPage extends StatefulWidget implements PageShape {
  @override
  final title = translate("Settings");

  @override
  final icon = Icon(Icons.settings);

  @override
  final appBarActions = [ScanButton()];

  @override
  State<SettingsPage> createState() => _SettingsState();
}

const url = 'https://rustdesk.com/';
final _hasIgnoreBattery = androidVersion >= 26;
var _ignoreBatteryOpt = false;
var _enableAbr = false;
var _denyLANDiscovery = false;
var _onlyWhiteList = false;
var _enableDirectIPAccess = false;
var _enableRecordSession = false;
var _autoRecordIncomingSession = false;
var _localIP = "";
var _directAccessPort = "";

class _SettingsState extends State<SettingsPage> with WidgetsBindingObserver {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);

    () async {
      var update = false;
      if (_hasIgnoreBattery) {
        update = await updateIgnoreBatteryStatus();
      }

      final enableAbrRes = await bind.mainGetOption(key: "enable-abr") != "N";
      if (enableAbrRes != _enableAbr) {
        update = true;
        _enableAbr = enableAbrRes;
      }

      final denyLanDiscovery = !option2bool('enable-lan-discovery',
          await bind.mainGetOption(key: 'enable-lan-discovery'));
      if (denyLanDiscovery != _denyLANDiscovery) {
        update = true;
        _denyLANDiscovery = denyLanDiscovery;
      }

      final onlyWhiteList =
          (await bind.mainGetOption(key: 'whitelist')).isNotEmpty;
      if (onlyWhiteList != _onlyWhiteList) {
        update = true;
        _onlyWhiteList = onlyWhiteList;
      }

      final enableDirectIPAccess = option2bool(
          'direct-server', await bind.mainGetOption(key: 'direct-server'));
      if (enableDirectIPAccess != _enableDirectIPAccess) {
        update = true;
        _enableDirectIPAccess = enableDirectIPAccess;
      }

      final enableRecordSession = option2bool('enable-record-session',
          await bind.mainGetOption(key: 'enable-record-session'));
      if (enableRecordSession != _enableRecordSession) {
        update = true;
        _enableRecordSession = enableRecordSession;
      }

      final autoRecordIncomingSession = option2bool(
          'allow-auto-record-incoming',
          await bind.mainGetOption(key: 'allow-auto-record-incoming'));
      if (autoRecordIncomingSession != _autoRecordIncomingSession) {
        update = true;
        _autoRecordIncomingSession = autoRecordIncomingSession;
      }

      final localIP = await bind.mainGetOption(key: 'local-ip-addr');
      if (localIP != _localIP) {
        update = true;
        _localIP = localIP;
      }

      final directAccessPort =
          await bind.mainGetOption(key: 'direct-access-port');
      if (directAccessPort != _directAccessPort) {
        update = true;
        _directAccessPort = directAccessPort;
      }

      if (update) {
        setState(() {});
      }
    }();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      () async {
        if (await updateIgnoreBatteryStatus()) {
          setState(() {});
        }
      }();
    }
  }

  Future<bool> updateIgnoreBatteryStatus() async {
    final res = await PermissionManager.check("ignore_battery_optimizations");
    if (_ignoreBatteryOpt != res) {
      _ignoreBatteryOpt = res;
      return true;
    } else {
      return false;
    }
  }

  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    final List<AbstractSettingsTile> enhancementsTiles = [];
    final List<AbstractSettingsTile> shareScreenTiles = [
      SettingsTile.switchTile(
        title: Text(translate('Deny LAN Discovery')),
        initialValue: _denyLANDiscovery,
        onToggle: (v) async {
          await bind.mainSetOption(
              key: "enable-lan-discovery",
              value: bool2option("enable-lan-discovery", !v));
          final newValue = !option2bool('enable-lan-discovery',
              await bind.mainGetOption(key: 'enable-lan-discovery'));
          setState(() {
            _denyLANDiscovery = newValue;
          });
        },
      ),
      SettingsTile.switchTile(
        title: Row(children: [
          Expanded(child: Text(translate('Use IP Whitelisting'))),
          Offstage(
                  offstage: !_onlyWhiteList,
                  child: const Icon(Icons.warning_amber_rounded,
                      color: Color.fromARGB(255, 255, 204, 0)))
              .marginOnly(left: 5)
        ]),
        initialValue: _onlyWhiteList,
        onToggle: (_) async {
          update() async {
            final onlyWhiteList =
                (await bind.mainGetOption(key: 'whitelist')).isNotEmpty;
            if (onlyWhiteList != _onlyWhiteList) {
              setState(() {
                _onlyWhiteList = onlyWhiteList;
              });
            }
          }

          changeWhiteList(callback: update);
        },
      ),
      SettingsTile.switchTile(
        title: Text('${translate('Adaptive Bitrate')} (beta)'),
        initialValue: _enableAbr,
        onToggle: (v) async {
          await bind.mainSetOption(key: "enable-abr", value: v ? "" : "N");
          final newValue = await bind.mainGetOption(key: "enable-abr") != "N";
          setState(() {
            _enableAbr = newValue;
          });
        },
      ),
      SettingsTile.switchTile(
        title: Text(translate('Enable Recording Session')),
        initialValue: _enableRecordSession,
        onToggle: (v) async {
          await bind.mainSetOption(
              key: "enable-record-session", value: v ? "" : "N");
          final newValue =
              await bind.mainGetOption(key: "enable-record-session") != "N";
          setState(() {
            _enableRecordSession = newValue;
          });
        },
      ),
      SettingsTile.switchTile(
        title: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Expanded(
                  child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                    Text(translate("Direct IP Access")),
                    Offstage(
                        offstage: !_enableDirectIPAccess,
                        child: Text(
                          '${translate("Local Address")}: $_localIP${_directAccessPort.isEmpty ? "" : ":$_directAccessPort"}',
                          style: Theme.of(context).textTheme.bodySmall,
                        )),
                  ])),
              Offstage(
                  offstage: !_enableDirectIPAccess,
                  child: IconButton(
                      padding: EdgeInsets.zero,
                      icon: Icon(
                        Icons.edit,
                        size: 20,
                      ),
                      onPressed: () async {
                        final port = await changeDirectAccessPort(
                            _localIP, _directAccessPort);
                        setState(() {
                          _directAccessPort = port;
                        });
                      }))
            ]),
        initialValue: _enableDirectIPAccess,
        onToggle: (_) async {
          _enableDirectIPAccess = !_enableDirectIPAccess;
          String value = bool2option('direct-server', _enableDirectIPAccess);
          await bind.mainSetOption(key: 'direct-server', value: value);
          setState(() {});
        },
      )
    ];
    if (_hasIgnoreBattery) {
      enhancementsTiles.insert(
          0,
          SettingsTile.switchTile(
              initialValue: _ignoreBatteryOpt,
              title: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(translate('Keep RustDesk background service')),
                    Text('* ${translate('Ignore Battery Optimizations')}',
                        style: Theme.of(context).textTheme.bodySmall),
                  ]),
              onToggle: (v) async {
                if (v) {
                  PermissionManager.request("ignore_battery_optimizations");
                } else {
                  final res = await gFFI.dialogManager
                      .show<bool>((setState, close) => CustomAlertDialog(
                            title: Text(translate("Open System Setting")),
                            content: Text(translate(
                                "android_open_battery_optimizations_tip")),
                            actions: [
                              TextButton(
                                  onPressed: () => close(),
                                  child: Text(translate("Cancel"))),
                              ElevatedButton(
                                  onPressed: () => close(true),
                                  child:
                                      Text(translate("Open System Setting"))),
                            ],
                          ));
                  if (res == true) {
                    PermissionManager.request("application_details_settings");
                  }
                }
              }));
    }

    return SettingsList(
      sections: [
        SettingsSection(
          title: Text(translate('Account')),
          tiles: [
            SettingsTile.navigation(
              title: Obx(() => Text(gFFI.userModel.userName.value.isEmpty
                  ? translate('Login')
                  : '${translate('Logout')} (${gFFI.userModel.userName.value})')),
              leading: Icon(Icons.person),
              onPressed: (context) {
                if (gFFI.userModel.userName.value.isEmpty) {
                  loginDialog();
                } else {
                  gFFI.userModel.logOut();
                }
              },
            ),
          ],
        ),
        SettingsSection(title: Text(translate("Settings")), tiles: [
          SettingsTile.navigation(
              title: Text(translate('ID/Relay Server')),
              leading: Icon(Icons.cloud),
              onPressed: (context) {
                showServerSettings(gFFI.dialogManager);
              }),
          SettingsTile.navigation(
              title: Text(translate('Language')),
              leading: Icon(Icons.translate),
              onPressed: (context) {
                showLanguageSettings(gFFI.dialogManager);
              }),
          SettingsTile.navigation(
            title: Text(translate('Dark Theme')),
            leading: Icon(Icons.dark_mode),
            onPressed: (context) {
              showThemeSettings(gFFI.dialogManager);
            },
          )
        ]),
        SettingsSection(
          title: Text(translate("Recording")),
          tiles: [
            SettingsTile.switchTile(
              title: Text(translate('Automatically record incoming sessions')),
              leading: Icon(Icons.videocam),
              description: FutureBuilder(
                  builder: (ctx, data) => Offstage(
                      offstage: !data.hasData,
                      child: Text("${translate("Directory")}: ${data.data}")),
                  future: bind.mainDefaultVideoSaveDirectory()),
              initialValue: _autoRecordIncomingSession,
              onToggle: (v) async {
                await bind.mainSetOption(
                    key: "allow-auto-record-incoming",
                    value: bool2option("allow-auto-record-incoming", v));
                final newValue = option2bool(
                    'allow-auto-record-incoming',
                    await bind.mainGetOption(
                        key: 'allow-auto-record-incoming'));
                setState(() {
                  _autoRecordIncomingSession = newValue;
                });
              },
            ),
          ],
        ),
        SettingsSection(
          title: Text(translate("Share Screen")),
          tiles: shareScreenTiles,
        ),
        SettingsSection(
          title: Text(translate("Enhancements")),
          tiles: enhancementsTiles,
        ),
        SettingsSection(
          title: Text(translate("About")),
          tiles: [
            SettingsTile.navigation(
                onPressed: (context) async {
                  if (await canLaunchUrl(Uri.parse(url))) {
                    await launchUrl(Uri.parse(url));
                  }
                },
                title: Text(translate("Version: ") + version),
                value: Padding(
                  padding: EdgeInsets.symmetric(vertical: 8),
                  child: Text('rustdesk.com',
                      style: TextStyle(
                        decoration: TextDecoration.underline,
                      )),
                ),
                leading: Icon(Icons.info)),
          ],
        ),
      ],
    );
  }
}

void showServerSettings(OverlayDialogManager dialogManager) async {
  Map<String, dynamic> options = jsonDecode(await bind.mainGetOptions());
  showServerSettingsWithValue(ServerConfig.fromOptions(options), dialogManager);
}

void showLanguageSettings(OverlayDialogManager dialogManager) async {
  try {
    final langs = json.decode(await bind.mainGetLangs()) as List<dynamic>;
    var lang = await bind.mainGetLocalOption(key: "lang");
    dialogManager.show((setState, close) {
      setLang(v) {
        if (lang != v) {
          setState(() {
            lang = v;
          });
          bind.mainSetLocalOption(key: "lang", value: v);
          HomePage.homeKey.currentState?.refreshPages();
          Future.delayed(Duration(milliseconds: 200), close);
        }
      }

      return CustomAlertDialog(
          title: SizedBox.shrink(),
          content: Column(
            children: [
                  getRadio('Default', '', lang, setLang),
                  Divider(color: MyTheme.border),
                ] +
                langs.map((e) {
                  final key = e[0] as String;
                  final name = e[1] as String;
                  return getRadio(name, key, lang, setLang);
                }).toList(),
          ),
          actions: []);
    }, backDismiss: true, clickMaskDismiss: true);
  } catch (e) {
    //
  }
}

void showThemeSettings(OverlayDialogManager dialogManager) async {
  var themeMode = MyTheme.getThemeModePreference();

  dialogManager.show((setState, close) {
    setTheme(v) {
      if (themeMode != v) {
        setState(() {
          themeMode = v;
        });
        MyTheme.changeDarkMode(themeMode);
        Future.delayed(Duration(milliseconds: 200), close);
      }
    }

    return CustomAlertDialog(
        title: SizedBox.shrink(),
        contentPadding: 10,
        content: Column(children: [
          getRadio('Light', ThemeMode.light, themeMode, setTheme),
          getRadio('Dark', ThemeMode.dark, themeMode, setTheme),
          getRadio('Follow System', ThemeMode.system, themeMode, setTheme)
        ]),
        actions: []);
  }, backDismiss: true, clickMaskDismiss: true);
}

void showAbout(OverlayDialogManager dialogManager) {
  dialogManager.show((setState, close) {
    return CustomAlertDialog(
      title: Text('${translate('About')} RustDesk'),
      content: Wrap(direction: Axis.vertical, spacing: 12, children: [
        Text('Version: $version'),
        InkWell(
            onTap: () async {
              const url = 'https://rustdesk.com/';
              if (await canLaunchUrl(Uri.parse(url))) {
                await launchUrl(Uri.parse(url));
              }
            },
            child: Padding(
              padding: EdgeInsets.symmetric(vertical: 8),
              child: Text('rustdesk.com',
                  style: TextStyle(
                    decoration: TextDecoration.underline,
                  )),
            )),
      ]),
      actions: [],
    );
  }, clickMaskDismiss: true, backDismiss: true);
}

class ScanButton extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return IconButton(
      icon: Icon(Icons.qr_code_scanner),
      onPressed: () {
        Navigator.push(
          context,
          MaterialPageRoute(
            builder: (BuildContext context) => ScanPage(),
          ),
        );
      },
    );
  }
}
