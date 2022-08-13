import 'dart:convert';
import 'dart:io' show Platform;
import 'dart:ui';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

const double _kCardFixedWidth = 600;
const double _kCardLeftPadding = 20;
const double _kContentLeftPadding = 30;
const double _kListViewBottomPadding = 30;

class DesktopSettingPage extends StatefulWidget {
  DesktopSettingPage({Key? key}) : super(key: key);

  @override
  State<DesktopSettingPage> createState() => _DesktopSettingPageState();
}

class _DesktopSettingPageState extends State<DesktopSettingPage>
    with TickerProviderStateMixin, AutomaticKeepAliveClientMixin {
  final List<NavigationRailDestination> _destinations =
      <NavigationRailDestination>[
    _destination('Display', Icons.palette_outlined, Icons.palette),
    _destination(
        'Security', Icons.health_and_safety_outlined, Icons.health_and_safety),
    _destination(
        'Connection', Icons.settings_remote_outlined, Icons.settings_remote),
    _destination('Video', Icons.videocam_outlined, Icons.videocam),
    _destination('Audio', Icons.volume_up_outlined, Icons.volume_up),
  ];

  late TabController controller;
  int _selectedIndex = 0;

  @override
  bool get wantKeepAlive => true;

  @override
  void initState() {
    super.initState();
    controller = TabController(length: _destinations.length, vsync: this);
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Scaffold(
      body: Row(
        children: <Widget>[
          NavigationRail(
            selectedIndex: _selectedIndex,
            onDestinationSelected: (int index) {
              setState(() {
                _selectedIndex = index;
              });
              controller.animateTo(index);
            },
            labelType: NavigationRailLabelType.all,
            destinations: _destinations,
          ),
          const VerticalDivider(thickness: 1, width: 1),
          Expanded(
            child: TabBarView(
              controller: controller,
              children: [
                _Display(),
                _Safety(),
                _Connection(),
                _Video(),
                _Audio(),
              ],
            ),
          )
        ],
      ),
    );
  }

  static NavigationRailDestination _destination(
      String label, IconData selected, IconData unSelected) {
    return NavigationRailDestination(
      icon: Icon(unSelected),
      selectedIcon: Icon(selected),
      label: Text(translate(label)),
    );
  }
}

//#region pages

class _Display extends StatefulWidget {
  _Display({Key? key}) : super(key: key);

  @override
  State<_Display> createState() => _DisplayState();
}

class _DisplayState extends State<_Display> with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return ListView(
      children: [
        _Card(title: translate('Display'), children: [language(), theme()]),
      ],
    ).paddingOnly(bottom: _kListViewBottomPadding);
  }

  Widget language() {
    return _futureBuilder(future: () async {
      String langs = await bind.mainGetLangs();
      String lang = await bind.mainGetLocalOption(key: "lang");
      return {"langs": langs, "lang": lang};
    }(), hasData: (res) {
      Map<String, String> data = res as Map<String, String>;
      List<dynamic> langsList = jsonDecode(data["langs"]!);
      Map<String, String> langsMap = {for (var v in langsList) v[0]: v[1]};
      List<String> keys = langsMap.keys.toList();
      List<String> values = langsMap.values.toList();
      keys.insert(0, "default");
      values.insert(0, "Default");
      String currentKey = data["lang"]!;
      if (!keys.contains(currentKey)) {
        currentKey = "default";
      }
      return _row(
          'Language',
          _ComboBox(
            keys: keys,
            values: values,
            initialKey: currentKey,
            onChanged: (key) async {
              await bind.mainSetLocalOption(key: "lang", value: key);
              Get.forceAppUpdate();
            },
          ));
    });
  }

  Widget theme() {
    return _row(
        'Dark Theme',
        Switch(
            value: isDarkTheme(),
            onChanged: ((dark) async {
              Get.changeTheme(dark ? MyTheme.darkTheme : MyTheme.lightTheme);
              Get.find<SharedPreferences>()
                  .setString("darkTheme", dark ? "Y" : "");
              Get.forceAppUpdate();
            })));
  }
}

class _Safety extends StatefulWidget {
  const _Safety({Key? key}) : super(key: key);

  @override
  State<_Safety> createState() => _SafetyState();
}

class _SafetyState extends State<_Safety> with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return ListView(
      children: [
        permissions(),
        password(),
        whitelist(),
      ],
    ).paddingOnly(bottom: _kListViewBottomPadding);
  }

  Widget permissions() {
    return _Card(title: 'Permissions', children: [
      _option_check('Enable Keyboard/Mouse', 'enable-keyboard'),
      _option_check('Enable Clipboard', 'enable-clipboard'),
      _option_check('Enable File Transfer', 'enable-file-transfer'),
      _option_check('Enable Audio', 'enable-audio'),
      _option_check('Enable Remote Restart', 'enable-remote-restart'),
      _option_check('Enable remote configuration modification',
          'allow-remote-config-modification'),
    ]);
  }

  Widget password() {
    return ChangeNotifierProvider.value(
        value: gFFI.serverModel,
        child: Consumer<ServerModel>(
            builder: ((context, model, child) =>
                _Card(title: 'Password', children: [
                  _row(
                      'Verification Method',
                      _ComboBox(
                          keys: [
                            kUseTemporaryPassword,
                            kUsePermanentPassword,
                            kUseBothPasswords,
                          ],
                          values: [
                            translate("Use temporary password"),
                            translate("Use permanent password"),
                            translate("Use both passwords"),
                          ],
                          initialKey: model.verificationMethod,
                          onChanged: (key) => model.verificationMethod = key)),
                  _row(
                      'Temporary Password Length',
                      _ComboBox(
                        keys: ['6', '8', '10'],
                        values: ['6', '8', '10'],
                        initialKey: model.temporaryPasswordLength,
                        onChanged: (key) => model.temporaryPasswordLength = key,
                        enabled:
                            model.verificationMethod != kUsePermanentPassword,
                      )),
                  _button(
                      'permanent_password_tip',
                      'Set permanent password',
                      setPasswordDialog,
                      model.verificationMethod != kUseTemporaryPassword)
                ]))));
  }

  Widget whitelist() {
    return _Card(title: 'IP Whitelisting', children: [
      _button('whitelist_tip', 'IP Whitelisting', changeWhiteList)
    ]);
  }
}

class _Connection extends StatefulWidget {
  const _Connection({Key? key}) : super(key: key);

  @override
  State<_Connection> createState() => _ConnectionState();
}

class _ConnectionState extends State<_Connection>
    with AutomaticKeepAliveClientMixin {
  final TextEditingController controller = TextEditingController();

  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return ListView(
      children: [
        _Card(title: 'Server', children: [
          _button('self-hosting_tip', 'ID/Relay Server', changeServer),
        ]),
        _Card(title: 'Service', children: [
          _option_check('Enable Service', 'stop-service', reverse: true),
          // TODO: Not implemented
          // _option_check('Always connected via relay', 'allow-always-relay'),
          // _option_check('Start ID/relay service', 'stop-rendezvous-service',
          //     reverse: true),
        ]),
        _Card(title: 'TCP Tunneling', children: [
          _option_check('Enable TCP Tunneling', 'enable-tunnel'),
        ]),
        direct_ip(),
        _Card(title: 'Proxy', children: [
          _button('socks5_proxy_tip', 'Socks5 Proxy', changeSocks5Proxy),
        ]),
      ],
    ).paddingOnly(bottom: _kListViewBottomPadding);
  }

  Widget direct_ip() {
    var update = () => setState(() {});
    return _Card(title: 'Direct IP Access', children: [
      _option_check('Enable Direct IP Access', 'direct-server', update: update),
      _row(
        'Port',
        _futureBuilder(
          future: () async {
            String enabled = await bind.mainGetOption(key: 'direct-server');
            String port = await bind.mainGetOption(key: 'direct-access-port');
            return {'enabled': enabled, 'port': port};
          }(),
          hasData: (data) {
            bool enabled =
                option2bool('direct-server', data['enabled'].toString());
            String port = data['port'].toString();
            int? iport = int.tryParse(port);
            if (iport == null || iport < 1 || iport > 65535) {
              port = '';
            }
            controller.text = port;
            return TextField(
              controller: controller,
              enabled: enabled,
              onChanged: (value) async {
                await bind.mainSetOption(
                    key: 'direct-access-port', value: controller.text);
              },
              decoration: InputDecoration(
                hintText: '21118',
              ),
            );
          },
        ),
      ),
    ]);
  }
}

class _Video extends StatefulWidget {
  const _Video({Key? key}) : super(key: key);

  @override
  State<_Video> createState() => _VideoState();
}

class _VideoState extends State<_Video> with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return ListView(
      children: [
        _Card(title: 'Adaptive Bitrate', children: [
          _option_check('Adaptive Bitrate', 'enable-abr'),
        ]),
      ],
    ).paddingOnly(bottom: _kListViewBottomPadding);
  }
}

class _Audio extends StatefulWidget {
  const _Audio({Key? key}) : super(key: key);

  @override
  State<_Audio> createState() => _AudioState();
}

class _AudioState extends State<_Audio> with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    var update = () => setState(() {});
    return ListView(children: [
      _Card(
        title: 'Audio Input',
        children: [
          _option_check('Mute', 'enable-audio', reverse: true, update: update),
          _row(
              'Audio device',
              _futureBuilder(future: () async {
                List<String> all = await bind.mainGetSoundInputs();
                String current = await bind.mainGetOption(key: 'audio-input');
                String enabled = await bind.mainGetOption(key: 'enable-audio');
                return {'all': all, 'current': current, 'enabled': enabled};
              }(), hasData: (data) {
                List<String> keys = (data['all'] as List<String>).toList();
                List<String> values = keys.toList();
                if (Platform.isWindows) {
                  keys.insert(0, '');
                  values.insert(0, 'System Sound');
                } else {
                  keys.insert(0, ''); // TODO
                  values.insert(0, 'None');
                }
                String initialKey = data['current'];
                if (!keys.contains(initialKey)) {
                  initialKey = '';
                }
                return _ComboBox(
                  keys: keys,
                  values: values,
                  initialKey: initialKey,
                  onChanged: (key) {
                    bind.mainSetOption(key: 'audio-input', value: key);
                  },
                  enabled:
                      option2bool('enable-audio', data['enabled'].toString()),
                );
              })),
        ],
      )
    ]).paddingOnly(bottom: _kListViewBottomPadding);
  }
}

//#endregion

//#region components

Widget _Card({required String title, required List<Widget> children}) {
  return Row(
    children: [
      Container(
        width: _kCardFixedWidth,
        child: Card(
          child: Column(
            children: [
              Row(
                children: [
                  Text(
                    translate(title),
                    textAlign: TextAlign.start,
                    style: TextStyle(
                      fontSize: 25,
                    ),
                  ),
                  Spacer(),
                ],
              ).paddingOnly(left: _kContentLeftPadding, top: 10, bottom: 20),
              ...children.map((e) => e.paddingOnly(top: 2)),
            ],
          ).paddingOnly(bottom: 10),
        ).paddingOnly(left: _kCardLeftPadding, top: 20),
      ),
    ],
  );
}

Widget _option_switch(String label, String key,
    {Function()? update = null, bool reverse = false}) {
  return _row(
      label,
      _futureBuilder(
          future: bind.mainGetOption(key: key),
          hasData: (data) {
            bool value = option2bool(key, data.toString());
            if (reverse) value = !value;
            var ref = value.obs;
            return Obx((() => Switch(
                value: ref.value,
                onChanged: ((option) async {
                  ref.value = option;
                  if (reverse) option = !option;
                  String value = bool2option(key, option);
                  bind.mainSetOption(key: key, value: value);
                  update?.call();
                }))));
          }));
}

Widget _option_check(String label, String key,
    {Function()? update = null, bool reverse = false}) {
  return Row(children: [
    _futureBuilder(
        future: bind.mainGetOption(key: key),
        hasData: (data) {
          bool value = option2bool(key, data.toString());
          if (reverse) value = !value;
          var ref = value.obs;
          return Obx((() => Checkbox(
              value: ref.value,
              onChanged: ((option) async {
                if (option != null) {
                  ref.value = option;
                  if (reverse) option = !option;
                  String value = bool2option(key, option);
                  bind.mainSetOption(key: key, value: value);
                  update?.call();
                }
              }))));
        }).paddingOnly(right: 10),
    Text(translate(label)),
  ]).paddingOnly(left: _kContentLeftPadding);
}

Widget _button(String tip, String label, Function() onPressed,
    [bool enabled = true]) {
  return _row(
      translate(tip),
      OutlinedButton(
          onPressed: enabled ? onPressed : null,
          child: Text(
            translate(label),
          )));
}

Widget _row(String label, Widget widget) {
  return Row(
    children: [
      Expanded(
          child: Text(
        translate(label),
      )),
      SizedBox(
        width: 40,
      ),
      Expanded(child: widget),
    ],
  ).paddingSymmetric(horizontal: _kContentLeftPadding);
}

Widget _futureBuilder(
    {required Future? future, required Widget Function(dynamic data) hasData}) {
  return FutureBuilder(
      future: future,
      builder: (BuildContext context, AsyncSnapshot snapshot) {
        if (snapshot.hasData) {
          return hasData(snapshot.data!);
        } else {
          if (snapshot.hasError) {
            print(snapshot.error.toString());
          }
          return Container();
        }
      });
}

class _ComboBox extends StatelessWidget {
  late final List<String> keys;
  late final List<String> values;
  late final String initialKey;
  late final Function(String key) onChanged;
  late final bool enabled;

  _ComboBox({
    Key? key,
    required this.keys,
    required this.values,
    required this.initialKey,
    required this.onChanged,
    this.enabled = true,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    var index = keys.indexOf(initialKey);
    if (index < 0) {
      assert(false);
      index = 0;
    }
    var ref = values[index].obs;
    return Container(
      child: SizedBox(
          child: Obx((() => DropdownButton<String>(
                isExpanded: true,
                value: ref.value,
                elevation: 16,
                underline: Container(
                  height: 40,
                ),
                icon: Icon(
                  Icons.arrow_drop_down_sharp,
                  size: 35,
                ),
                onChanged: enabled
                    ? (String? newValue) {
                        if (newValue != null && newValue != ref.value) {
                          ref.value = newValue;
                          onChanged(keys[values.indexOf(newValue)]);
                        }
                      }
                    : null,
                items: values.map<DropdownMenuItem<String>>((String value) {
                  return DropdownMenuItem<String>(
                    value: value,
                    child: Text(value),
                  );
                }).toList(),
              )))),
    );
  }
}

//#endregion

//#region dialogs

void changeServer() async {
  Map<String, dynamic> oldOptions = jsonDecode(await bind.mainGetOptions());
  print("${oldOptions}");
  String idServer = oldOptions['custom-rendezvous-server'] ?? "";
  var idServerMsg = "";
  String relayServer = oldOptions['relay-server'] ?? "";
  var relayServerMsg = "";
  String apiServer = oldOptions['api-server'] ?? "";
  var apiServerMsg = "";
  var key = oldOptions['key'] ?? "";

  var isInProgress = false;
  gFFI.dialogManager.show((setState, close) {
    return CustomAlertDialog(
      title: Text(translate("ID/Relay Server")),
      content: ConstrainedBox(
        constraints: BoxConstraints(minWidth: 500),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: BoxConstraints(minWidth: 100),
                    child: Text("${translate('ID Server')}:")
                        .marginOnly(bottom: 16.0)),
                SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    onChanged: (s) {
                      idServer = s;
                    },
                    decoration: InputDecoration(
                        border: OutlineInputBorder(),
                        errorText: idServerMsg.isNotEmpty ? idServerMsg : null),
                    controller: TextEditingController(text: idServer),
                  ),
                ),
              ],
            ),
            SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: BoxConstraints(minWidth: 100),
                    child: Text("${translate('Relay Server')}:")
                        .marginOnly(bottom: 16.0)),
                SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    onChanged: (s) {
                      relayServer = s;
                    },
                    decoration: InputDecoration(
                        border: OutlineInputBorder(),
                        errorText:
                            relayServerMsg.isNotEmpty ? relayServerMsg : null),
                    controller: TextEditingController(text: relayServer),
                  ),
                ),
              ],
            ),
            SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: BoxConstraints(minWidth: 100),
                    child: Text("${translate('API Server')}:")
                        .marginOnly(bottom: 16.0)),
                SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    onChanged: (s) {
                      apiServer = s;
                    },
                    decoration: InputDecoration(
                        border: OutlineInputBorder(),
                        errorText:
                            apiServerMsg.isNotEmpty ? apiServerMsg : null),
                    controller: TextEditingController(text: apiServer),
                  ),
                ),
              ],
            ),
            SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: BoxConstraints(minWidth: 100),
                    child:
                        Text("${translate('Key')}:").marginOnly(bottom: 16.0)),
                SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    onChanged: (s) {
                      key = s;
                    },
                    decoration: InputDecoration(
                      border: OutlineInputBorder(),
                    ),
                    controller: TextEditingController(text: key),
                  ),
                ),
              ],
            ),
            SizedBox(
              height: 4.0,
            ),
            Offstage(offstage: !isInProgress, child: LinearProgressIndicator())
          ],
        ),
      ),
      actions: [
        TextButton(
            onPressed: () {
              close();
            },
            child: Text(translate("Cancel"))),
        TextButton(
            onPressed: () async {
              setState(() {
                [idServerMsg, relayServerMsg, apiServerMsg].forEach((element) {
                  element = "";
                });
                isInProgress = true;
              });
              final cancel = () {
                setState(() {
                  isInProgress = false;
                });
              };
              idServer = idServer.trim();
              relayServer = relayServer.trim();
              apiServer = apiServer.trim();
              key = key.trim();

              if (idServer.isNotEmpty) {
                idServerMsg = translate(
                    await bind.mainTestIfValidServer(server: idServer));
                if (idServerMsg.isEmpty) {
                  oldOptions['custom-rendezvous-server'] = idServer;
                } else {
                  cancel();
                  return;
                }
              } else {
                oldOptions['custom-rendezvous-server'] = "";
              }

              if (relayServer.isNotEmpty) {
                relayServerMsg = translate(
                    await bind.mainTestIfValidServer(server: relayServer));
                if (relayServerMsg.isEmpty) {
                  oldOptions['relay-server'] = relayServer;
                } else {
                  cancel();
                  return;
                }
              } else {
                oldOptions['relay-server'] = "";
              }

              if (apiServer.isNotEmpty) {
                if (apiServer.startsWith('http://') ||
                    apiServer.startsWith("https://")) {
                  oldOptions['api-server'] = apiServer;
                  return;
                } else {
                  apiServerMsg = translate("invalid_http");
                  cancel();
                  return;
                }
              } else {
                oldOptions['api-server'] = "";
              }
              // ok
              oldOptions['key'] = key;
              await bind.mainSetOptions(json: jsonEncode(oldOptions));
              close();
            },
            child: Text(translate("OK"))),
      ],
    );
  });
}

void changeWhiteList() async {
  Map<String, dynamic> oldOptions = jsonDecode(await bind.mainGetOptions());
  var newWhiteList = ((oldOptions['whitelist'] ?? "") as String).split(',');
  var newWhiteListField = newWhiteList.join('\n');
  var msg = "";
  var isInProgress = false;
  gFFI.dialogManager.show((setState, close) {
    return CustomAlertDialog(
      title: Text(translate("IP Whitelisting")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(translate("whitelist_sep")),
          SizedBox(
            height: 8.0,
          ),
          Row(
            children: [
              Expanded(
                child: TextField(
                  onChanged: (s) {
                    newWhiteListField = s;
                  },
                  maxLines: null,
                  decoration: InputDecoration(
                    border: OutlineInputBorder(),
                    errorText: msg.isEmpty ? null : translate(msg),
                  ),
                  controller: TextEditingController(text: newWhiteListField),
                ),
              ),
            ],
          ),
          SizedBox(
            height: 4.0,
          ),
          Offstage(offstage: !isInProgress, child: LinearProgressIndicator())
        ],
      ),
      actions: [
        TextButton(
            onPressed: () {
              close();
            },
            child: Text(translate("Cancel"))),
        TextButton(
            onPressed: () async {
              setState(() {
                msg = "";
                isInProgress = true;
              });
              newWhiteListField = newWhiteListField.trim();
              var newWhiteList = "";
              if (newWhiteListField.isEmpty) {
                // pass
              } else {
                final ips =
                    newWhiteListField.trim().split(RegExp(r"[\s,;\n]+"));
                // test ip
                final ipMatch = RegExp(r"^\d+\.\d+\.\d+\.\d+$");
                for (final ip in ips) {
                  if (!ipMatch.hasMatch(ip)) {
                    msg = translate("Invalid IP") + " $ip";
                    setState(() {
                      isInProgress = false;
                    });
                    return;
                  }
                }
                newWhiteList = ips.join(',');
              }
              oldOptions['whitelist'] = newWhiteList;
              await bind.mainSetOptions(json: jsonEncode(oldOptions));
              close();
            },
            child: Text(translate("OK"))),
      ],
    );
  });
}

void changeSocks5Proxy() async {
  var socks = await bind.mainGetSocks();

  String proxy = "";
  String proxyMsg = "";
  String username = "";
  String password = "";
  if (socks.length == 3) {
    proxy = socks[0];
    username = socks[1];
    password = socks[2];
  }

  var isInProgress = false;
  gFFI.dialogManager.show((setState, close) {
    return CustomAlertDialog(
      title: Text(translate("Socks5 Proxy")),
      content: ConstrainedBox(
        constraints: BoxConstraints(minWidth: 500),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: BoxConstraints(minWidth: 100),
                    child: Text("${translate('Hostname')}:")
                        .marginOnly(bottom: 16.0)),
                SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    onChanged: (s) {
                      proxy = s;
                    },
                    decoration: InputDecoration(
                        border: OutlineInputBorder(),
                        errorText: proxyMsg.isNotEmpty ? proxyMsg : null),
                    controller: TextEditingController(text: proxy),
                  ),
                ),
              ],
            ),
            SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: BoxConstraints(minWidth: 100),
                    child: Text("${translate('Username')}:")
                        .marginOnly(bottom: 16.0)),
                SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    onChanged: (s) {
                      username = s;
                    },
                    decoration: InputDecoration(
                      border: OutlineInputBorder(),
                    ),
                    controller: TextEditingController(text: username),
                  ),
                ),
              ],
            ),
            SizedBox(
              height: 8.0,
            ),
            Row(
              children: [
                ConstrainedBox(
                    constraints: BoxConstraints(minWidth: 100),
                    child: Text("${translate('Password')}:")
                        .marginOnly(bottom: 16.0)),
                SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    onChanged: (s) {
                      password = s;
                    },
                    decoration: InputDecoration(
                      border: OutlineInputBorder(),
                    ),
                    controller: TextEditingController(text: password),
                  ),
                ),
              ],
            ),
            SizedBox(
              height: 8.0,
            ),
            Offstage(offstage: !isInProgress, child: LinearProgressIndicator())
          ],
        ),
      ),
      actions: [
        TextButton(
            onPressed: () {
              close();
            },
            child: Text(translate("Cancel"))),
        TextButton(
            onPressed: () async {
              setState(() {
                proxyMsg = "";
                isInProgress = true;
              });
              final cancel = () {
                setState(() {
                  isInProgress = false;
                });
              };
              proxy = proxy.trim();
              username = username.trim();
              password = password.trim();

              if (proxy.isNotEmpty) {
                proxyMsg =
                    translate(await bind.mainTestIfValidServer(server: proxy));
                if (proxyMsg.isEmpty) {
                  // ignore
                } else {
                  cancel();
                  return;
                }
              }
              await bind.mainSetSocks(
                  proxy: proxy, username: username, password: password);
              close();
            },
            child: Text(translate("OK"))),
      ],
    );
  });
}

//#endregion
