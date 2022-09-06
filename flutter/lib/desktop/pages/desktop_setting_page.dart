import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:url_launcher/url_launcher_string.dart';

const double _kTabWidth = 235;
const double _kTabHeight = 42;
const double _kCardFixedWidth = 560;
const double _kCardLeftMargin = 15;
const double _kContentHMargin = 15;
const double _kContentHSubMargin = _kContentHMargin + 33;
const double _kCheckBoxLeftMargin = 10;
const double _kRadioLeftMargin = 10;
const double _kListViewBottomMargin = 15;
const double _kTitleFontSize = 20;
const double _kContentFontSize = 15;
const Color _accentColor = MyTheme.accent;

class _TabInfo {
  late final String label;
  late final IconData unselected;
  late final IconData selected;
  _TabInfo(this.label, this.unselected, this.selected);
}

class DesktopSettingPage extends StatefulWidget {
  DesktopSettingPage({Key? key}) : super(key: key);

  @override
  State<DesktopSettingPage> createState() => _DesktopSettingPageState();
}

class _DesktopSettingPageState extends State<DesktopSettingPage>
    with TickerProviderStateMixin, AutomaticKeepAliveClientMixin {
  final List<_TabInfo> _setting_tabs = <_TabInfo>[
    _TabInfo('User Interface', Icons.language_outlined, Icons.language_sharp),
    _TabInfo('Security', Icons.enhanced_encryption_outlined,
        Icons.enhanced_encryption_sharp),
    _TabInfo(
        'Display', Icons.desktop_windows_outlined, Icons.desktop_windows_sharp),
    _TabInfo('Audio', Icons.volume_up_outlined, Icons.volume_up_sharp),
    _TabInfo('Connection', Icons.link_outlined, Icons.link_sharp),
    _TabInfo('About', Icons.info_outline, Icons.info_sharp)
  ];

  late PageController controller;
  RxInt _selectedIndex = 0.obs;

  @override
  bool get wantKeepAlive => true;

  @override
  void initState() {
    super.initState();
    controller = PageController();
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Scaffold(
      backgroundColor: MyTheme.color(context).bg,
      body: Row(
        children: <Widget>[
          Container(
            width: _kTabWidth,
            child: Column(
              children: [
                _header(),
                Flexible(child: _listView(tabs: _setting_tabs)),
              ],
            ),
          ),
          const VerticalDivider(thickness: 1, width: 1),
          Expanded(
            child: Container(
              color: MyTheme.color(context).grayBg,
              child: PageView(
                controller: controller,
                children: [
                  _UserInterface(),
                  _Safety(),
                  _Display(),
                  _Audio(),
                  _Connection(),
                  _About(),
                ],
              ),
            ),
          )
        ],
      ),
    );
  }

  Widget _header() {
    return Row(
      children: [
        SizedBox(
          height: 62,
          child: Text(
            translate('Settings'),
            textAlign: TextAlign.left,
            style: TextStyle(
              color: _accentColor,
              fontSize: _kTitleFontSize,
              fontWeight: FontWeight.w400,
            ),
          ),
        ).marginOnly(left: 20, top: 10),
        Spacer(),
      ],
    );
  }

  Widget _listView({required List<_TabInfo> tabs}) {
    return ListView(
      children: tabs
          .asMap()
          .entries
          .map((tab) => _listItem(tab: tab.value, index: tab.key))
          .toList(),
    );
  }

  Widget _listItem({required _TabInfo tab, required int index}) {
    return Obx(() {
      bool selected = index == _selectedIndex.value;
      return Container(
        width: _kTabWidth,
        height: _kTabHeight,
        child: InkWell(
          onTap: () {
            if (_selectedIndex.value != index) {
              controller.jumpToPage(index);
            }
            _selectedIndex.value = index;
          },
          child: Row(children: [
            Container(
              width: 4,
              height: _kTabHeight * 0.7,
              color: selected ? _accentColor : null,
            ),
            Icon(
              selected ? tab.selected : tab.unselected,
              color: selected ? _accentColor : null,
              size: 20,
            ).marginOnly(left: 13, right: 10),
            Text(
              translate(tab.label),
              style: TextStyle(
                  color: selected ? _accentColor : null,
                  fontWeight: FontWeight.w400,
                  fontSize: _kContentFontSize),
            ),
          ]),
        ),
      );
    });
  }
}

//#region pages

class _UserInterface extends StatefulWidget {
  _UserInterface({Key? key}) : super(key: key);

  @override
  State<_UserInterface> createState() => _UserInterfaceState();
}

class _UserInterfaceState extends State<_UserInterface>
    with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return ListView(
      children: [
        _Card(title: 'Language', children: [language()]),
        _Card(title: 'Theme', children: [theme()]),
      ],
    ).marginOnly(bottom: _kListViewBottomMargin);
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
      return _ComboBox(
        keys: keys,
        values: values,
        initialKey: currentKey,
        onChanged: (key) async {
          await bind.mainSetLocalOption(key: "lang", value: key);
          Get.forceAppUpdate();
        },
      ).marginOnly(left: _kContentHMargin);
    });
  }

  Widget theme() {
    change() {
      MyTheme.changeTo(!isDarkTheme());
    }

    return GestureDetector(
      onTap: change,
      child: Row(
        children: [
          Checkbox(value: isDarkTheme(), onChanged: (_) => change()),
          Expanded(child: Text(translate('Dark Theme'))),
        ],
      ).marginOnly(left: _kCheckBoxLeftMargin),
    );
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
  bool locked = true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return ListView(
      children: [
        Column(
          children: [
            _lock(locked, 'Unlock Security Settings', () {
              locked = false;
              setState(() => {});
            }),
            AbsorbPointer(
              absorbing: locked,
              child: Column(children: [
                permissions(context),
                password(context),
                whitelist(),
              ]),
            ),
          ],
        )
      ],
    ).marginOnly(bottom: _kListViewBottomMargin);
  }

  Widget permissions(context) {
    bool enabled = !locked;
    return _Card(title: 'Permissions', children: [
      _OptionCheckBox(context, 'Enable Keyboard/Mouse', 'enable-keyboard',
          enabled: enabled),
      _OptionCheckBox(context, 'Enable Clipboard', 'enable-clipboard',
          enabled: enabled),
      _OptionCheckBox(context, 'Enable File Transfer', 'enable-file-transfer',
          enabled: enabled),
      _OptionCheckBox(context, 'Enable Audio', 'enable-audio',
          enabled: enabled),
      _OptionCheckBox(context, 'Enable Remote Restart', 'enable-remote-restart',
          enabled: enabled),
      _OptionCheckBox(context, 'Enable remote configuration modification',
          'allow-remote-config-modification',
          enabled: enabled),
    ]);
  }

  Widget password(BuildContext context) {
    return ChangeNotifierProvider.value(
        value: gFFI.serverModel,
        child: Consumer<ServerModel>(builder: ((context, model, child) {
          List<String> keys = [
            kUseTemporaryPassword,
            kUsePermanentPassword,
            kUseBothPasswords,
          ];
          List<String> values = [
            translate("Use temporary password"),
            translate("Use permanent password"),
            translate("Use both passwords"),
          ];
          bool tmpEnabled = model.verificationMethod != kUsePermanentPassword;
          bool permEnabled = model.verificationMethod != kUseTemporaryPassword;
          String currentValue = values[keys.indexOf(model.verificationMethod)];
          List<Widget> radios = values
              .map((value) => _Radio<String>(
                    context,
                    value: value,
                    groupValue: currentValue,
                    label: value,
                    onChanged: ((value) {
                      () async {
                        await model
                            .setVerificationMethod(keys[values.indexOf(value)]);
                        await model.updatePasswordModel();
                      }();
                    }),
                    enabled: !locked,
                  ))
              .toList();

          var onChanged = tmpEnabled && !locked
              ? (value) {
                  if (value != null) {
                    () async {
                      await model.setTemporaryPasswordLength(value.toString());
                      await model.updatePasswordModel();
                    }();
                  }
                }
              : null;
          List<Widget> lengthRadios = ['6', '8', '10']
              .map((value) => GestureDetector(
                    child: Row(
                      children: [
                        Radio(
                            value: value,
                            groupValue: model.temporaryPasswordLength,
                            onChanged: onChanged),
                        Text(
                          value,
                          style: TextStyle(
                              color: _disabledTextColor(
                                  context, onChanged != null)),
                        ),
                      ],
                    ).paddingSymmetric(horizontal: 10),
                    onTap: () => onChanged?.call(value),
                  ))
              .toList();

          return _Card(title: 'Password', children: [
            radios[0],
            _SubLabeledWidget(
                'Temporary Password Length',
                Row(
                  children: [
                    ...lengthRadios,
                  ],
                ),
                enabled: tmpEnabled && !locked),
            radios[1],
            _SubButton('Set permanent password', setPasswordDialog,
                permEnabled && !locked),
            radios[2],
          ]);
        })));
  }

  Widget whitelist() {
    return _Card(title: 'IP Whitelisting', children: [
      _Button('IP Whitelisting', changeWhiteList,
          tip: 'whitelist_tip', enabled: !locked)
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
  @override
  bool get wantKeepAlive => true;
  bool locked = true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    bool enabled = !locked;
    return ListView(children: [
      Column(
        children: [
          _lock(locked, 'Unlock Connection Settings', () {
            locked = false;
            setState(() => {});
          }),
          AbsorbPointer(
            absorbing: locked,
            child: Column(children: [
              _Card(title: 'Server', children: [
                _Button('ID/Relay Server', changeServer, enabled: enabled),
              ]),
              _Card(title: 'Service', children: [
                _OptionCheckBox(context, 'Enable Service', 'stop-service',
                    reverse: true, enabled: enabled),
                // TODO: Not implemented
                // _option_check('Always connected via relay', 'allow-always-relay', enabled: enabled),
                // _option_check('Start ID/relay service', 'stop-rendezvous-service',
                //     reverse: true, enabled: enabled),
              ]),
              _Card(title: 'TCP Tunneling', children: [
                _OptionCheckBox(
                    context, 'Enable TCP Tunneling', 'enable-tunnel',
                    enabled: enabled),
              ]),
              direct_ip(context),
              _Card(title: 'Proxy', children: [
                _Button('Socks5 Proxy', changeSocks5Proxy, enabled: enabled),
              ]),
            ]),
          ),
        ],
      )
    ]).marginOnly(bottom: _kListViewBottomMargin);
  }

  Widget direct_ip(BuildContext context) {
    TextEditingController controller = TextEditingController();
    var update = () => setState(() {});
    RxBool apply_enabled = false.obs;
    return _Card(title: 'Direct IP Access', children: [
      _OptionCheckBox(context, 'Enable Direct IP Access', 'direct-server',
          update: update, enabled: !locked),
      _futureBuilder(
        future: () async {
          String enabled = await bind.mainGetOption(key: 'direct-server');
          String port = await bind.mainGetOption(key: 'direct-access-port');
          return {'enabled': enabled, 'port': port};
        }(),
        hasData: (data) {
          bool enabled =
              option2bool('direct-server', data['enabled'].toString());
          if (!enabled) apply_enabled.value = false;
          controller.text = data['port'].toString();
          return Row(children: [
            _SubLabeledWidget(
              'Port',
              Container(
                width: 80,
                child: TextField(
                  controller: controller,
                  enabled: enabled && !locked,
                  onChanged: (_) => apply_enabled.value = true,
                  inputFormatters: [
                    FilteringTextInputFormatter.allow(RegExp(
                        '\^([0-9]|[1-9]\\d|[1-9]\\d{2}|[1-9]\\d{3}|[1-5]\\d{4}|6[0-4]\\d{3}|65[0-4]\\d{2}|655[0-2]\\d|6553[0-5])\$')),
                  ],
                  textAlign: TextAlign.end,
                  decoration: InputDecoration(
                    hintText: '21118',
                    border: InputBorder.none,
                    contentPadding: EdgeInsets.only(right: 5),
                    isCollapsed: true,
                  ),
                ),
              ),
              enabled: enabled && !locked,
            ).marginOnly(left: 5),
            Obx(() => ElevatedButton(
                  onPressed: apply_enabled.value && enabled && !locked
                      ? () async {
                          apply_enabled.value = false;
                          await bind.mainSetOption(
                              key: 'direct-access-port',
                              value: controller.text);
                        }
                      : null,
                  child: Text(
                    translate('Apply'),
                  ),
                ).marginOnly(left: 20))
          ]);
        },
      ),
    ]);
  }
}

class _Display extends StatefulWidget {
  const _Display({Key? key}) : super(key: key);

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
        _Card(title: 'Adaptive Bitrate', children: [
          _OptionCheckBox(context, 'Adaptive Bitrate', 'enable-abr'),
        ]),
        hwcodec(),
      ],
    ).marginOnly(bottom: _kListViewBottomMargin);
  }

  Widget hwcodec() {
    return _futureBuilder(
        future: bind.mainHasHwcodec(),
        hasData: (data) {
          return Offstage(
            offstage: !(data as bool),
            child: _Card(title: 'Hardware Codec', children: [
              _OptionCheckBox(
                  context, 'Enable hardware codec', 'enable-hwcodec'),
            ]),
          );
        });
  }
}

class _Audio extends StatefulWidget {
  const _Audio({Key? key}) : super(key: key);

  @override
  State<_Audio> createState() => _AudioState();
}

enum _AudioInputType {
  Mute,
  Standard,
  Specify,
}

class _AudioState extends State<_Audio> with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    var update = () => setState(() {});
    var set_enabled = (bool enabled) => bind.mainSetOption(
        key: 'enable-audio', value: bool2option('enable-audio', enabled));
    var set_device = (String device) =>
        bind.mainSetOption(key: 'audio-input', value: device);
    return ListView(children: [
      _Card(
        title: 'Audio Input',
        children: [
          _futureBuilder(future: () async {
            List<String> devices = await bind.mainGetSoundInputs();
            String current = await bind.mainGetOption(key: 'audio-input');
            String enabled = await bind.mainGetOption(key: 'enable-audio');
            return {'devices': devices, 'current': current, 'enabled': enabled};
          }(), hasData: (data) {
            bool mute =
                !option2bool('enable-audio', data['enabled'].toString());
            String currentDevice = data['current'];
            List<String> devices = (data['devices'] as List<String>).toList();
            _AudioInputType groupValue;
            if (mute) {
              groupValue = _AudioInputType.Mute;
            } else if (devices.contains(currentDevice)) {
              groupValue = _AudioInputType.Specify;
            } else {
              groupValue = _AudioInputType.Standard;
            }
            List deviceWidget = [].toList();
            if (devices.isNotEmpty) {
              var combo = _ComboBox(
                keys: devices,
                values: devices,
                initialKey: devices.contains(currentDevice)
                    ? currentDevice
                    : devices[0],
                onChanged: (key) {
                  set_device(key);
                },
                enabled: groupValue == _AudioInputType.Specify,
              );
              deviceWidget.addAll([
                _Radio<_AudioInputType>(
                  context,
                  value: _AudioInputType.Specify,
                  groupValue: groupValue,
                  label: 'Specify device',
                  onChanged: (value) {
                    set_device(combo.current);
                    set_enabled(true);
                    update();
                  },
                ),
                combo.marginOnly(left: _kContentHSubMargin, top: 5),
              ]);
            }
            return Column(children: [
              _Radio<_AudioInputType>(
                context,
                value: _AudioInputType.Mute,
                groupValue: groupValue,
                label: 'Mute',
                onChanged: (value) {
                  set_enabled(false);
                  update();
                },
              ),
              _Radio(
                context,
                value: _AudioInputType.Standard,
                groupValue: groupValue,
                label: 'Use standard device',
                onChanged: (value) {
                  set_device('');
                  set_enabled(true);
                  update();
                },
              ),
              ...deviceWidget,
            ]);
          }),
        ],
      )
    ]).marginOnly(bottom: _kListViewBottomMargin);
  }
}

class _About extends StatefulWidget {
  const _About({Key? key}) : super(key: key);

  @override
  State<_About> createState() => _AboutState();
}

class _AboutState extends State<_About> {
  @override
  Widget build(BuildContext context) {
    return _futureBuilder(future: () async {
      final license = await bind.mainGetLicense();
      final version = await bind.mainGetVersion();
      return {'license': license, 'version': version};
    }(), hasData: (data) {
      final license = data['license'].toString();
      final version = data['version'].toString();
      final linkStyle = TextStyle(decoration: TextDecoration.underline);
      return ListView(children: [
        _Card(title: "About RustDesk", children: [
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SizedBox(
                height: 8.0,
              ),
              Text("Version: $version").marginSymmetric(vertical: 4.0),
              InkWell(
                  onTap: () {
                    launchUrlString("https://rustdesk.com/privacy");
                  },
                  child: Text(
                    "Privacy Statement",
                    style: linkStyle,
                  ).marginSymmetric(vertical: 4.0)),
              InkWell(
                  onTap: () {
                    launchUrlString("https://rustdesk.com");
                  },
                  child: Text(
                    "Website",
                    style: linkStyle,
                  ).marginSymmetric(vertical: 4.0)),
              Container(
                decoration: BoxDecoration(color: Color(0xFF2c8cff)),
                padding: EdgeInsets.symmetric(vertical: 24, horizontal: 8),
                child: Row(
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            "Copyright &copy; 2022 Purslane Ltd.\n$license",
                            style: TextStyle(color: Colors.white),
                          ),
                          Text(
                            "Made with heart in this chaotic world!",
                            style: TextStyle(
                                fontWeight: FontWeight.w800,
                                color: Colors.white),
                          )
                        ],
                      ),
                    ),
                  ],
                ),
              ).marginSymmetric(vertical: 4.0)
            ],
          ).marginOnly(left: _kContentHMargin)
        ]),
      ]);
    });
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
                      fontSize: _kTitleFontSize,
                    ),
                  ),
                  Spacer(),
                ],
              ).marginOnly(left: _kContentHMargin, top: 10, bottom: 10),
              ...children
                  .map((e) => e.marginOnly(top: 4, right: _kContentHMargin)),
            ],
          ).marginOnly(bottom: 10),
        ).marginOnly(left: _kCardLeftMargin, top: 15),
      ),
    ],
  );
}

Color? _disabledTextColor(BuildContext context, bool enabled) {
  return enabled ? null : MyTheme.color(context).lighterText;
}

Widget _OptionCheckBox(BuildContext context, String label, String key,
    {Function()? update = null, bool reverse = false, bool enabled = true}) {
  return _futureBuilder(
      future: bind.mainGetOption(key: key),
      hasData: (data) {
        bool value = option2bool(key, data.toString());
        if (reverse) value = !value;
        var ref = value.obs;
        var onChanged = (option) async {
          if (option != null) {
            ref.value = option;
            if (reverse) option = !option;
            String value = bool2option(key, option);
            bind.mainSetOption(key: key, value: value);
            update?.call();
          }
        };
        return GestureDetector(
          child: Obx(
            () => Row(
              children: [
                Checkbox(
                        value: ref.value, onChanged: enabled ? onChanged : null)
                    .marginOnly(right: 10),
                Expanded(
                    child: Text(
                  translate(label),
                  style: TextStyle(color: _disabledTextColor(context, enabled)),
                ))
              ],
            ),
          ).marginOnly(left: _kCheckBoxLeftMargin),
          onTap: () {
            onChanged(!ref.value);
          },
        );
      });
}

Widget _Radio<T>(BuildContext context,
    {required T value,
    required T groupValue,
    required String label,
    required Function(T value) onChanged,
    bool enabled = true}) {
  var on_change = enabled
      ? (T? value) {
          if (value != null) {
            onChanged(value);
          }
        }
      : null;
  return GestureDetector(
    child: Row(
      children: [
        Radio<T>(value: value, groupValue: groupValue, onChanged: on_change),
        Expanded(
          child: Text(translate(label),
                  style: TextStyle(
                      fontSize: _kContentFontSize,
                      color: _disabledTextColor(context, enabled)))
              .marginOnly(left: 5),
        ),
      ],
    ).marginOnly(left: _kRadioLeftMargin),
    onTap: () => on_change?.call(value),
  );
}

Widget _Button(String label, Function() onPressed,
    {bool enabled = true, String? tip}) {
  var button = ElevatedButton(
      onPressed: enabled ? onPressed : null,
      child: Container(
        child: Text(
          translate(label),
        ).marginSymmetric(horizontal: 15),
      ));
  var child;
  if (tip == null) {
    child = button;
  } else {
    child = Tooltip(message: translate(tip), child: button);
  }
  return Row(children: [
    child,
  ]).marginOnly(left: _kContentHMargin);
}

Widget _SubButton(String label, Function() onPressed, [bool enabled = true]) {
  return Row(
    children: [
      ElevatedButton(
          onPressed: enabled ? onPressed : null,
          child: Container(
            child: Text(
              translate(label),
            ).marginSymmetric(horizontal: 15),
          )),
    ],
  ).marginOnly(left: _kContentHSubMargin);
}

Widget _SubLabeledWidget(String label, Widget child, {bool enabled = true}) {
  RxBool hover = false.obs;
  return Row(
    children: [
      MouseRegion(
          onEnter: (_) => hover.value = true,
          onExit: (_) => hover.value = false,
          child: Obx(
            () {
              return Container(
                  height: 32,
                  decoration: BoxDecoration(
                      border: Border.all(
                          color: hover.value && enabled
                              ? Color(0xFFD7D7D7)
                              : Color(0xFFCBCBCB),
                          width: hover.value && enabled ? 2 : 1)),
                  child: Row(
                    children: [
                      Container(
                        height: 28,
                        color: (hover.value && enabled)
                            ? Color(0xFFD7D7D7)
                            : Color(0xFFCBCBCB),
                        child: Text(
                          label + ': ',
                          style: TextStyle(fontWeight: FontWeight.w300),
                        ),
                        alignment: Alignment.center,
                        padding:
                            EdgeInsets.symmetric(horizontal: 5, vertical: 2),
                      ).paddingAll(2),
                      child,
                    ],
                  ));
            },
          )),
    ],
  ).marginOnly(left: _kContentHSubMargin);
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

Widget _lock(
  bool locked,
  String label,
  Function() onUnlock,
) {
  return Offstage(
      offstage: !locked,
      child: Row(
        children: [
          Container(
            width: _kCardFixedWidth,
            child: Card(
              child: ElevatedButton(
                child: Container(
                    height: 25,
                    child: Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Icon(
                            Icons.security_sharp,
                            size: 20,
                          ),
                          Text(translate(label)).marginOnly(left: 5),
                        ]).marginSymmetric(vertical: 2)),
                onPressed: () async {
                  bool checked = await bind.mainCheckSuperUserPermission();
                  if (checked) {
                    onUnlock();
                  }
                },
              ).marginSymmetric(horizontal: 2, vertical: 4),
            ).marginOnly(left: _kCardLeftMargin),
          ).marginOnly(top: 10),
        ],
      ));
}

// ignore: must_be_immutable
class _ComboBox extends StatelessWidget {
  late final List<String> keys;
  late final List<String> values;
  late final String initialKey;
  late final Function(String key) onChanged;
  late final bool enabled;
  late String current;

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
    current = keys[index];
    return Container(
      decoration: BoxDecoration(border: Border.all(color: MyTheme.border)),
      height: 30,
      child: Obx(() => DropdownButton<String>(
            isExpanded: true,
            value: ref.value,
            elevation: 16,
            underline: Container(
              height: 25,
            ),
            icon: Icon(
              Icons.expand_more_sharp,
              size: 20,
            ),
            onChanged: enabled
                ? (String? newValue) {
                    if (newValue != null && newValue != ref.value) {
                      ref.value = newValue;
                      current = newValue;
                      onChanged(keys[values.indexOf(newValue)]);
                    }
                  }
                : null,
            items: values.map<DropdownMenuItem<String>>((String value) {
              return DropdownMenuItem<String>(
                value: value,
                child: Text(
                  value,
                  style: TextStyle(fontSize: _kContentFontSize),
                  overflow: TextOverflow.ellipsis,
                ).marginOnly(left: 5),
              );
            }).toList(),
          )),
    );
  }
}

//#endregion

//#region dialogs

void changeServer() async {
  Map<String, dynamic> oldOptions = jsonDecode(await bind.mainGetOptions());
  String idServer = oldOptions['custom-rendezvous-server'] ?? "";
  var idServerMsg = "";
  String relayServer = oldOptions['relay-server'] ?? "";
  var relayServerMsg = "";
  String apiServer = oldOptions['api-server'] ?? "";
  var apiServerMsg = "";
  var key = oldOptions['key'] ?? "";
  var idController = TextEditingController(text: idServer);
  var relayController = TextEditingController(text: relayServer);
  var apiController = TextEditingController(text: apiServer);
  var keyController = TextEditingController(text: key);

  var isInProgress = false;

  gFFI.dialogManager.show((setState, close) {
    submit() async {
      setState(() {
        [idServerMsg, relayServerMsg, apiServerMsg].forEach((element) {
          element = "";
        });
        isInProgress = true;
      });
      cancel() {
        setState(() {
          isInProgress = false;
        });
      }

      idServer = idController.text.trim();
      relayServer = relayController.text.trim();
      apiServer = apiController.text.trim().toLowerCase();
      key = keyController.text.trim();

      if (idServer.isNotEmpty) {
        idServerMsg =
            translate(await bind.mainTestIfValidServer(server: idServer));
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
        relayServerMsg =
            translate(await bind.mainTestIfValidServer(server: relayServer));
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
    }

    return CustomAlertDialog(
      title: Text(translate("ID/Relay Server")),
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
                    child: Text("${translate('ID Server')}:")
                        .marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    decoration: InputDecoration(
                        border: const OutlineInputBorder(),
                        errorText: idServerMsg.isNotEmpty ? idServerMsg : null),
                    controller: idController,
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
                    child: Text("${translate('Relay Server')}:")
                        .marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    decoration: InputDecoration(
                        border: const OutlineInputBorder(),
                        errorText:
                            relayServerMsg.isNotEmpty ? relayServerMsg : null),
                    controller: relayController,
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
                    child: Text("${translate('API Server')}:")
                        .marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    decoration: InputDecoration(
                        border: const OutlineInputBorder(),
                        errorText:
                            apiServerMsg.isNotEmpty ? apiServerMsg : null),
                    controller: apiController,
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
                    child:
                        Text("${translate('Key')}:").marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    decoration: const InputDecoration(
                      border: OutlineInputBorder(),
                    ),
                    controller: keyController,
                  ),
                ),
              ],
            ),
            const SizedBox(
              height: 4.0,
            ),
            Offstage(
                offstage: !isInProgress, child: const LinearProgressIndicator())
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

void changeWhiteList() async {
  Map<String, dynamic> oldOptions = jsonDecode(await bind.mainGetOptions());
  var newWhiteList = ((oldOptions['whitelist'] ?? "") as String).split(',');
  var newWhiteListField = newWhiteList.join('\n');
  var controller = TextEditingController(text: newWhiteListField);
  var msg = "";
  var isInProgress = false;
  gFFI.dialogManager.show((setState, close) {
    return CustomAlertDialog(
      title: Text(translate("IP Whitelisting")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(translate("whitelist_sep")),
          const SizedBox(
            height: 8.0,
          ),
          Row(
            children: [
              Expanded(
                child: TextField(
                    maxLines: null,
                    decoration: InputDecoration(
                      border: const OutlineInputBorder(),
                      errorText: msg.isEmpty ? null : translate(msg),
                    ),
                    controller: controller,
                    focusNode: FocusNode()..requestFocus()),
              ),
            ],
          ),
          const SizedBox(
            height: 4.0,
          ),
          Offstage(
              offstage: !isInProgress, child: const LinearProgressIndicator())
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
              newWhiteListField = controller.text.trim();
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
                    msg = "${translate("Invalid IP")} $ip";
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
      onCancel: close,
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
  var proxyController = TextEditingController(text: proxy);
  var userController = TextEditingController(text: username);
  var pwdController = TextEditingController(text: password);

  var isInProgress = false;
  gFFI.dialogManager.show((setState, close) {
    submit() async {
      setState(() {
        proxyMsg = "";
        isInProgress = true;
      });
      cancel() {
        setState(() {
          isInProgress = false;
        });
      }

      proxy = proxyController.text.trim();
      username = userController.text.trim();
      password = pwdController.text.trim();

      if (proxy.isNotEmpty) {
        proxyMsg = translate(await bind.mainTestIfValidServer(server: proxy));
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
    }

    return CustomAlertDialog(
      title: Text(translate("Socks5 Proxy")),
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
                    child: Text("${translate('Hostname')}:")
                        .marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    decoration: InputDecoration(
                        border: const OutlineInputBorder(),
                        errorText: proxyMsg.isNotEmpty ? proxyMsg : null),
                    controller: proxyController,
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
                    child: Text("${translate('Username')}:")
                        .marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    decoration: const InputDecoration(
                      border: OutlineInputBorder(),
                    ),
                    controller: userController,
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
                    child: Text("${translate('Password')}:")
                        .marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: TextField(
                    decoration: const InputDecoration(
                      border: OutlineInputBorder(),
                    ),
                    controller: pwdController,
                  ),
                ),
              ],
            ),
            const SizedBox(
              height: 8.0,
            ),
            Offstage(
                offstage: !isInProgress, child: const LinearProgressIndicator())
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

//#endregion
