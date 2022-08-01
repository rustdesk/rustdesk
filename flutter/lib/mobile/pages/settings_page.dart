import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'package:provider/provider.dart';
import 'package:settings_ui/settings_ui.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../common.dart';
import '../../models/model.dart';
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

class _SettingsState extends State<SettingsPage> with WidgetsBindingObserver {
  static const url = 'https://rustdesk.com/';
  final _hasIgnoreBattery = androidVersion >= 26;
  var _ignoreBatteryOpt = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    if (_hasIgnoreBattery) {
      updateIgnoreBatteryStatus();
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      updateIgnoreBatteryStatus();
    }
  }

  Future<bool> updateIgnoreBatteryStatus() async {
    final res = await PermissionManager.check("ignore_battery_optimizations");
    if (_ignoreBatteryOpt != res) {
      setState(() {
        _ignoreBatteryOpt = res;
      });
      return true;
    } else {
      return false;
    }
  }

  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    final username = getUsername();
    final enableAbr = gFFI.getByName("option", "enable-abr") != 'N';
    final enhancementsTiles = [
      SettingsTile.switchTile(
        title: Text(translate('Adaptive Bitrate') + '(beta)'),
        initialValue: enableAbr,
        onToggle: (v) {
          final msg = Map()
            ..["name"] = "enable-abr"
            ..["value"] = "";
          if (!v) {
            msg["value"] = "N";
          }
          gFFI.setByName("option", json.encode(msg));
          setState(() {});
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
                  final res = await DialogManager.show<bool>(
                      (setState, close) => CustomAlertDialog(
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
                  showLogin();
                } else {
                  logout();
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
                showServerSettings();
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

void showServerSettings() {
  final id = gFFI.getByName('option', 'custom-rendezvous-server');
  final relay = gFFI.getByName('option', 'relay-server');
  final api = gFFI.getByName('option', 'api-server');
  final key = gFFI.getByName('option', 'key');
  showServerSettingsWithValue(id, relay, key, api);
}

void showAbout() {
  DialogManager.show((setState, close) {
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
    'id': gFFI.getByName('server_id'),
    'uuid': gFFI.getByName('uuid')
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
    gFFI.setByName('option', '{"name": "access_token", "value": "$token"}');
  }
  final info = data['user'];
  if (info != null) {
    final value = json.encode(info);
    gFFI.setByName(
        'option', json.encode({"name": "user_info", "value": value}));
    gFFI.ffiModel.updateUser();
  }
  return '';
}

void refreshCurrentUser() async {
  final token = gFFI.getByName("option", "access_token");
  if (token == '') return;
  final url = getUrl();
  final body = {
    'id': gFFI.getByName('server_id'),
    'uuid': gFFI.getByName('uuid')
  };
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

void logout() async {
  final token = gFFI.getByName("option", "access_token");
  if (token == '') return;
  final url = getUrl();
  final body = {
    'id': gFFI.getByName('server_id'),
    'uuid': gFFI.getByName('uuid')
  };
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

void resetToken() {
  gFFI.setByName('option', '{"name": "access_token", "value": ""}');
  gFFI.setByName('option', '{"name": "user_info", "value": ""}');
  gFFI.ffiModel.updateUser();
}

String getUrl() {
  var url = gFFI.getByName('option', 'api-server');
  if (url == '') {
    url = gFFI.getByName('option', 'custom-rendezvous-server');
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

void showLogin() {
  final passwordController = TextEditingController();
  final nameController = TextEditingController();
  var loading = false;
  var error = '';
  DialogManager.show((setState, close) {
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

String? getUsername() {
  final token = gFFI.getByName("option", "access_token");
  String? username;
  if (token != "") {
    final info = gFFI.getByName("option", "user_info");
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
