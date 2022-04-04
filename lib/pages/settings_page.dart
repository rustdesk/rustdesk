import 'package:flutter_easyloading/flutter_easyloading.dart';
import 'package:settings_ui/settings_ui.dart';
import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:provider/provider.dart';
import 'dart:convert';
import 'package:http/http.dart' as http;
import '../common.dart';
import '../widgets/dialog.dart';
import '../models/model.dart';
import 'home_page.dart';

class SettingsPage extends StatefulWidget implements PageShape {
  @override
  final title = translate("Settings");

  @override
  final icon = Icon(Icons.settings);

  @override
  final appBarActions = [];

  @override
  _SettingsState createState() => _SettingsState();
}

class _SettingsState extends State<SettingsPage> {
  static const url = 'https://rustdesk.com/';

  @override
  Widget build(BuildContext context) {
    Provider.of<FfiModel>(context);
    final username = getUsername();
    return SettingsList(
      contentPadding: EdgeInsets.symmetric(horizontal: 12),
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
        SettingsSection(
          title: Text(translate("Settings")),
          tiles: [
            SettingsTile.navigation(
              title: Text(translate('ID/Relay Server')),
              leading: Icon(Icons.cloud),
              onPressed: (context) {
                showServer();
              },
            ),
          ],
        ),
        SettingsSection(
          title: Text(translate("About")),
          tiles: [
            SettingsTile.navigation(
                title: Text(translate("Version: ") + version),
                value: InkWell(
                  onTap: () async {
                    if (await canLaunch(url)) {
                      await launch(url);
                    }
                  },
                  child: Padding(
                    padding: EdgeInsets.symmetric(vertical: 8),
                    child: Text('rustdesk.com',
                        style: TextStyle(
                          decoration: TextDecoration.underline,
                        )),
                  ),
                ),
                leading: Icon(Icons.info)),
          ],
        ),
      ],
    );
  }
}

void showServer() {
  final formKey = GlobalKey<FormState>();
  final id0 = FFI.getByName('option', 'custom-rendezvous-server');
  final relay0 = FFI.getByName('option', 'relay-server');
  final api0 = FFI.getByName('option', 'api-server');
  final key0 = FFI.getByName('option', 'key');
  var id = '';
  var relay = '';
  var key = '';
  var api = '';
  DialogManager.show((setState, close) {
    return CustomAlertDialog(
      title: Text(translate('ID/Relay Server')),
      content: Form(
          key: formKey,
          child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                    TextFormField(
                      initialValue: id0,
                      decoration: InputDecoration(
                        labelText: translate('ID Server'),
                      ),
                      validator: validate,
                      onSaved: (String? value) {
                        if (value != null) id = value.trim();
                      },
                    )
                  ] +
                  (isAndroid
                      ? [
                          TextFormField(
                            initialValue: relay0,
                            decoration: InputDecoration(
                              labelText: translate('Relay Server'),
                            ),
                            validator: validate,
                            onSaved: (String? value) {
                              if (value != null) relay = value.trim();
                            },
                          )
                        ]
                      : []) +
                  [
                    TextFormField(
                      initialValue: api0,
                      decoration: InputDecoration(
                        labelText: translate('API Server'),
                      ),
                      validator: validate,
                      onSaved: (String? value) {
                        if (value != null) api = value.trim();
                      },
                    ),
                    TextFormField(
                      initialValue: key0,
                      decoration: InputDecoration(
                        labelText: 'Key',
                      ),
                      validator: null,
                      onSaved: (String? value) {
                        if (value != null) key = value.trim();
                      },
                    ),
                  ])),
      actions: [
        TextButton(
          style: flatButtonStyle,
          onPressed: () {
            close();
          },
          child: Text(translate('Cancel')),
        ),
        TextButton(
          style: flatButtonStyle,
          onPressed: () {
            if (formKey.currentState != null &&
                formKey.currentState!.validate()) {
              formKey.currentState!.save();
              if (id != id0)
                FFI.setByName('option',
                    '{"name": "custom-rendezvous-server", "value": "$id"}');
              if (relay != relay0)
                FFI.setByName(
                    'option', '{"name": "relay-server", "value": "$relay"}');
              if (key != key0)
                FFI.setByName('option', '{"name": "key", "value": "$key"}');
              if (api != api0)
                FFI.setByName(
                    'option', '{"name": "api-server", "value": "$api"}');
              close();
            }
          },
          child: Text(translate('OK')),
        ),
      ],
      onWillPop: () async {
        return true;
      },
    );
  }, barrierDismissible: true);
}

String? validate(value) {
  value = value.trim();
  if (value.isEmpty) {
    return null;
  }
  final res = FFI.getByName('test_if_valid_server', value);
  return res.isEmpty ? null : res;
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
              if (await canLaunch(url)) {
                await launch(url);
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
      onWillPop: () async {
        return true;
      },
    );
  }, barrierDismissible: true);
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
    'id': FFI.getByName('server_id'),
    'uuid': FFI.getByName('uuid')
  };
  try {
    final response = await http.post(Uri.parse('${url}/api/login'),
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
    FFI.setByName('option', '{"name": "access_token", "value": "$token"}');
  }
  final info = data['user'];
  if (info != null) {
    final value = json.encode(info);
    FFI.setByName('option', json.encode({"name": "user_info", "value": value}));
    FFI.ffiModel.updateUser();
  }
  return '';
}

void refreshCurrentUser() async {
  final token = FFI.getByName("option", "access_token");
  if (token == '') return;
  final url = getUrl();
  final body = {
    'id': FFI.getByName('server_id'),
    'uuid': FFI.getByName('uuid')
  };
  try {
    final response = await http.post(Uri.parse('${url}/api/currentUser'),
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
  final token = FFI.getByName("option", "access_token");
  if (token == '') return;
  final url = getUrl();
  final body = {
    'id': FFI.getByName('server_id'),
    'uuid': FFI.getByName('uuid')
  };
  try {
    await http.post(Uri.parse('${url}/api/logout'),
        headers: {
          "Content-Type": "application/json",
          "Authorization": "Bearer $token"
        },
        body: json.encode(body));
  } catch (e) {
    EasyLoading.showToast('Failed to access $url',
        maskType: EasyLoadingMaskType.black);
  }
  resetToken();
}

void resetToken() {
  FFI.setByName('option', '{"name": "access_token", "value": ""}');
  FFI.setByName('option', '{"name": "user_info", "value": ""}');
  FFI.ffiModel.updateUser();
}

String getUrl() {
  var url = FFI.getByName('option', 'api-server');
  if (url == '') {
    url = FFI.getByName('option', 'custom-rendezvous-server');
    if (url != '') {
      if (url.contains(':')) {
        final tmp = url.split(':');
        if (tmp.length == 2) {
          var port = int.parse(tmp[1]) - 2;
          url = 'http://${tmp[0]}:$port';
        }
      } else {
        url = 'http://${url}:21114';
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
  final token = FFI.getByName("option", "access_token");
  String? username;
  if (token != "") {
    final info = FFI.getByName("option", "user_info");
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
