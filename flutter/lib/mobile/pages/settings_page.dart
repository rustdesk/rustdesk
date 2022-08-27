import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'package:provider/provider.dart';
import 'package:settings_ui/settings_ui.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../common.dart';
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
  _SettingsState createState() => _SettingsState();
}

const url = 'https://rustdesk.com/';
final _hasIgnoreBattery = androidVersion >= 26;
var _ignoreBatteryOpt = false;
var _enableAbr = false;

class _SettingsState extends State<SettingsPage> with WidgetsBindingObserver {
  String? username;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);

    () async {
      var update = false;
      if (_hasIgnoreBattery) {
        update = await updateIgnoreBatteryStatus();
      }

      final usernameRes = await getUsername();
      if (usernameRes != username) {
        update = true;
        username = usernameRes;
      }

      final enableAbrRes = await bind.mainGetOption(key: "enable-abr") != "N";
      if (enableAbrRes != _enableAbr) {
        update = true;
        _enableAbr = enableAbrRes;
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
    final enhancementsTiles = [
      SettingsTile.switchTile(
        title: Text(translate('Adaptive Bitrate') + ' (beta)'),
        initialValue: _enableAbr,
        onToggle: (v) {
          bind.mainSetOption(key: "enable-abr", value: v ? "" : "N");
          setState(() {
            _enableAbr = !_enableAbr;
          });
        },
      )
    ];
    if (_hasIgnoreBattery) {
      enhancementsTiles.insert(
          0,
          SettingsTile.switchTile(
              initialValue: _ignoreBatteryOpt,
              title: Text(translate('Keep RustDesk background service')),
              description:
                  Text('* ${translate('Ignore Battery Optimizations')}'),
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
          title: Text(translate("Account")),
          tiles: [
            SettingsTile.navigation(
              title: Text(username == null
                  ? translate("Login")
                  : translate("Logout") + ' ($username)'),
              leading: Icon(Icons.person),
              onPressed: (context) {
                if (username == null) {
                  showLogin(gFFI.dialogManager);
                } else {
                  logout(gFFI.dialogManager);
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
              })
        ]),
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
  String id = options['custom-rendezvous-server'] ?? "";
  String relay = options['relay-server'] ?? "";
  String api = options['api-server'] ?? "";
  String key = options['key'] ?? "";
  showServerSettingsWithValue(id, relay, key, api, dialogManager);
}

void showLanguageSettings(OverlayDialogManager dialogManager) async {
  try {
    final langs = json.decode(await bind.mainGetLangs()) as List<dynamic>;
    var lang = await bind.mainGetLocalOption(key: "lang");
    dialogManager.show((setState, close) {
      final setLang = (v) {
        if (lang != v) {
          setState(() {
            lang = v;
          });
          bind.mainSetLocalOption(key: "lang", value: v);
          HomePage.homeKey.currentState?.refreshPages();
          Future.delayed(Duration(milliseconds: 200), close);
        }
      };
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
  } catch (_e) {}
}

void showAbout(OverlayDialogManager dialogManager) {
  dialogManager.show((setState, close) {
    return CustomAlertDialog(
      title: Text(translate('About') + ' RustDesk'),
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

Future<String> login(String name, String pass) async {
/* js test CORS
const data = { username: 'example', password: 'xx' };

fetch('http://localhost:21114/api/login', {
  method: 'POST', // or 'PUT'
  headers: {
    'Content-Type': 'application/json',
  },
  body: JSON.stringify(data),
})
.then(response => response.json())
.then(data => {
  console.log('Success:', data);
})
.catch((error) => {
  console.error('Error:', error);
});
*/
  final url = getUrl();
  final body = {
    'username': name,
    'password': pass,
    'id': bind.mainGetMyId(),
    'uuid': bind.mainGetUuid()
  };
  try {
    final response = await http.post(Uri.parse('$url/api/login'),
        headers: {"Content-Type": "application/json"}, body: json.encode(body));
    return parseResp(response.body);
  } catch (e) {
    print(e);
    return 'Failed to access $url';
  }
}

String parseResp(String body) {
  final data = json.decode(body);
  final error = data['error'];
  if (error != null) {
    return error!;
  }
  final token = data['access_token'];
  if (token != null) {
    bind.mainSetOption(key: "access_token", value: token);
  }
  final info = data['user'];
  if (info != null) {
    final value = json.encode(info);
    bind.mainSetOption(key: "user_info", value: value);
    gFFI.ffiModel.updateUser();
  }
  return '';
}

void refreshCurrentUser() async {
  final token = await bind.mainGetOption(key: "access_token");
  if (token == '') return;
  final url = getUrl();
  final body = {'id': bind.mainGetMyId(), 'uuid': bind.mainGetUuid()};
  try {
    final response = await http.post(Uri.parse('$url/api/currentUser'),
        headers: {
          "Content-Type": "application/json",
          "Authorization": "Bearer $token"
        },
        body: json.encode(body));
    final status = response.statusCode;
    if (status == 401 || status == 400) {
      resetToken();
      return;
    }
    parseResp(response.body);
  } catch (e) {
    print('$e');
  }
}

void logout(OverlayDialogManager dialogManager) async {
  final token = await bind.mainGetOption(key: "access_token");
  if (token == '') return;
  final url = getUrl();
  final body = {'id': bind.mainGetMyId(), 'uuid': bind.mainGetUuid()};
  try {
    await http.post(Uri.parse('$url/api/logout'),
        headers: {
          "Content-Type": "application/json",
          "Authorization": "Bearer $token"
        },
        body: json.encode(body));
  } catch (e) {
    showToast('Failed to access $url');
  }
  resetToken();
}

void resetToken() async {
  await bind.mainSetOption(key: "access_token", value: "");
  await bind.mainSetOption(key: "user_info", value: "");
  gFFI.ffiModel.updateUser();
}

Future<String> getUrl() async {
  var url = await bind.mainGetOption(key: "api-server");
  if (url == '') {
    url = await bind.mainGetOption(key: "custom-rendezvous-server");
    if (url != '') {
      if (url.contains(':')) {
        final tmp = url.split(':');
        if (tmp.length == 2) {
          var port = int.parse(tmp[1]) - 2;
          url = 'http://${tmp[0]}:$port';
        }
      } else {
        url = 'http://$url:21114';
      }
    }
  }
  if (url == '') {
    url = 'https://admin.rustdesk.com';
  }
  return url;
}

void showLogin(OverlayDialogManager dialogManager) {
  final passwordController = TextEditingController();
  final nameController = TextEditingController();
  var loading = false;
  var error = '';
  dialogManager.show((setState, close) {
    return CustomAlertDialog(
      title: Text(translate('Login')),
      content: Column(mainAxisSize: MainAxisSize.min, children: [
        TextField(
          autofocus: true,
          autocorrect: false,
          enableSuggestions: false,
          keyboardType: TextInputType.visiblePassword,
          decoration: InputDecoration(
            labelText: translate('Username'),
          ),
          controller: nameController,
        ),
        PasswordWidget(controller: passwordController),
      ]),
      actions: (loading
              ? <Widget>[CircularProgressIndicator()]
              : (error != ""
                  ? <Widget>[
                      Text(translate(error),
                          style: TextStyle(color: Colors.red))
                    ]
                  : <Widget>[])) +
          <Widget>[
            TextButton(
              style: flatButtonStyle,
              onPressed: loading
                  ? null
                  : () {
                      close();
                      setState(() {
                        loading = false;
                      });
                    },
              child: Text(translate('Cancel')),
            ),
            TextButton(
              style: flatButtonStyle,
              onPressed: loading
                  ? null
                  : () async {
                      final name = nameController.text.trim();
                      final pass = passwordController.text.trim();
                      if (name != "" && pass != "") {
                        setState(() {
                          loading = true;
                        });
                        final e = await login(name, pass);
                        setState(() {
                          loading = false;
                          error = e;
                        });
                        if (e == "") {
                          close();
                        }
                      }
                    },
              child: Text(translate('OK')),
            ),
          ],
    );
  });
}

Future<String?> getUsername() async {
  final token = await bind.mainGetOption(key: "access_token");
  String? username;
  if (token != "") {
    final info = await bind.mainGetOption(key: "user_info");
    if (info != "") {
      try {
        Map<String, dynamic> tmp = json.decode(info);
        username = tmp["name"];
      } catch (e) {
        print('$e');
      }
    }
  }
  return username;
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
