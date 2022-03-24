import 'package:settings_ui/settings_ui.dart';
import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';
import '../common.dart';
import '../models/model.dart';
import 'home_page.dart';

class SettingsPage extends StatelessWidget implements PageShape {
  @override
  final title = translate("Settings");

  @override
  final icon = Icon(Icons.settings);

  @override
  final appBarActions = [];

  static const url = 'https://rustdesk.com/';

  @override
  Widget build(BuildContext context) {
    return SettingsList(
      sections: [
        SettingsSection(
          title: Text("Common"),
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
          title: Text("About"),
          tiles: [
            SettingsTile.navigation(
                title: Text("Version: " + version),
                value: InkWell(
                  onTap: () async {
                    const url = 'https://rustdesk.com/';
                    if (await canLaunch(url)) {
                      await launch(url);
                    }
                  },
                  child: Padding(
                    padding: EdgeInsets.symmetric(vertical: 8),
                    child: Text('Support',
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
  final key0 = FFI.getByName('option', 'key');
  var id = '';
  var relay = '';
  var key = '';
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
              child: Text('Support',
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
