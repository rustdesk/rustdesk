import 'dart:convert';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/pages/desktop_tab_page.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:url_launcher/url_launcher_string.dart';
import 'package:flutter_hbb/desktop/widgets/scroll_wrapper.dart';

import '../../common/widgets/dialog.dart';
import '../../common/widgets/login.dart';

const double _kTabWidth = 235;
const double _kTabHeight = 42;
const double _kCardFixedWidth = 540;
const double _kCardLeftMargin = 15;
const double _kContentHMargin = 15;
const double _kContentHSubMargin = _kContentHMargin + 33;
const double _kCheckBoxLeftMargin = 10;
const double _kRadioLeftMargin = 10;
const double _kListViewBottomMargin = 15;
const double _kTitleFontSize = 20;
const double _kContentFontSize = 15;
const Color _accentColor = MyTheme.accent;
const String _kSettingPageControllerTag = 'settingPageController';
const String _kSettingPageIndexTag = 'settingPageIndex';
const int _kPageCount = 6;

class _TabInfo {
  late final String label;
  late final IconData unselected;
  late final IconData selected;
  _TabInfo(this.label, this.unselected, this.selected);
}

class DesktopSettingPage extends StatefulWidget {
  final int initialPage;

  const DesktopSettingPage({Key? key, required this.initialPage})
      : super(key: key);

  @override
  State<DesktopSettingPage> createState() => _DesktopSettingPageState();

  static void switch2page(int page) {
    if (page >= _kPageCount) return;
    try {
      if (Get.isRegistered<PageController>(tag: _kSettingPageControllerTag)) {
        DesktopTabPage.onAddSetting(initialPage: page);
        PageController controller = Get.find(tag: _kSettingPageControllerTag);
        RxInt selectedIndex = Get.find(tag: _kSettingPageIndexTag);
        selectedIndex.value = page;
        controller.jumpToPage(page);
      } else {
        DesktopTabPage.onAddSetting(initialPage: page);
      }
    } catch (e) {
      debugPrintStack(label: '$e');
    }
  }
}

class _DesktopSettingPageState extends State<DesktopSettingPage>
    with TickerProviderStateMixin, AutomaticKeepAliveClientMixin {
  final List<_TabInfo> settingTabs = <_TabInfo>[
    _TabInfo('General', Icons.settings_outlined, Icons.settings),
    _TabInfo('Security', Icons.enhanced_encryption_outlined,
        Icons.enhanced_encryption),
    _TabInfo('Network', Icons.link_outlined, Icons.link),
    _TabInfo('Display', Icons.desktop_windows_outlined, Icons.desktop_windows),
    _TabInfo('Account', Icons.person_outline, Icons.person),
    _TabInfo('About', Icons.info_outline, Icons.info)
  ];

  late PageController controller;
  late RxInt selectedIndex;

  @override
  bool get wantKeepAlive => true;

  @override
  void initState() {
    super.initState();
    selectedIndex =
        (widget.initialPage < _kPageCount ? widget.initialPage : 0).obs;
    Get.put<RxInt>(selectedIndex, tag: _kSettingPageIndexTag);
    controller = PageController(initialPage: widget.initialPage);
    Get.put<PageController>(controller, tag: _kSettingPageControllerTag);
  }

  @override
  void dispose() {
    super.dispose();
    Get.delete<PageController>(tag: _kSettingPageControllerTag);
    Get.delete<RxInt>(tag: _kSettingPageIndexTag);
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return Scaffold(
      backgroundColor: Theme.of(context).backgroundColor,
      body: Row(
        children: <Widget>[
          SizedBox(
            width: _kTabWidth,
            child: Column(
              children: [
                _header(),
                Flexible(child: _listView(tabs: settingTabs)),
              ],
            ),
          ),
          const VerticalDivider(thickness: 1, width: 1),
          Expanded(
            child: Container(
              color: Theme.of(context).scaffoldBackgroundColor,
              child: DesktopScrollWrapper(
                  scrollController: controller,
                  child: PageView(
                    controller: controller,
                    physics: DraggableNeverScrollableScrollPhysics(),
                    children: const [
                      _General(),
                      _Safety(),
                      _Network(),
                      _Display(),
                      _Account(),
                      _About(),
                    ],
                  )),
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
            style: const TextStyle(
              color: _accentColor,
              fontSize: _kTitleFontSize,
              fontWeight: FontWeight.w400,
            ),
          ),
        ).marginOnly(left: 20, top: 10),
        const Spacer(),
      ],
    );
  }

  Widget _listView({required List<_TabInfo> tabs}) {
    final scrollController = ScrollController();
    return DesktopScrollWrapper(
        scrollController: scrollController,
        child: ListView(
          physics: DraggableNeverScrollableScrollPhysics(),
          controller: scrollController,
          children: tabs
              .asMap()
              .entries
              .map((tab) => _listItem(tab: tab.value, index: tab.key))
              .toList(),
        ));
  }

  Widget _listItem({required _TabInfo tab, required int index}) {
    return Obx(() {
      bool selected = index == selectedIndex.value;
      return SizedBox(
        width: _kTabWidth,
        height: _kTabHeight,
        child: InkWell(
          onTap: () {
            if (selectedIndex.value != index) {
              controller.jumpToPage(index);
            }
            selectedIndex.value = index;
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

class _General extends StatefulWidget {
  const _General({Key? key}) : super(key: key);

  @override
  State<_General> createState() => _GeneralState();
}

class _GeneralState extends State<_General> {
  @override
  Widget build(BuildContext context) {
    final scrollController = ScrollController();
    return DesktopScrollWrapper(
        scrollController: scrollController,
        child: ListView(
          physics: DraggableNeverScrollableScrollPhysics(),
          controller: scrollController,
          children: [
            theme(),
            hwcodec(),
            audio(context),
            record(context),
            _Card(title: 'Language', children: [language()]),
            other()
          ],
        ).marginOnly(bottom: _kListViewBottomMargin));
  }

  Widget theme() {
    final current = MyTheme.getThemeModePreference().toShortString();
    onChanged(String value) {
      MyTheme.changeDarkMode(MyTheme.themeModeFromString(value));
      setState(() {});
    }

    return _Card(title: 'Theme', children: [
      _Radio<String>(context,
          value: 'light',
          groupValue: current,
          label: 'Light',
          onChanged: onChanged),
      _Radio<String>(context,
          value: 'dark',
          groupValue: current,
          label: 'Dark',
          onChanged: onChanged),
      _Radio<String>(context,
          value: 'system',
          groupValue: current,
          label: 'Follow System',
          onChanged: onChanged),
    ]);
  }

  Widget other() {
    return _Card(title: 'Other', children: [
      _OptionCheckBox(context, 'Confirm before closing multiple tabs',
          'enable-confirm-closing-tabs'),
      _OptionCheckBox(context, 'Adaptive Bitrate', 'enable-abr'),
      if (Platform.isLinux)
        Tooltip(
          message: translate('software_render_tip'),
          child: _OptionCheckBox(
            context,
            "Always use software rendering",
            'allow-always-software-render',
          ),
        )
    ]);
  }

  Widget hwcodec() {
    return Offstage(
      offstage: !bind.mainHasHwcodec(),
      child: _Card(title: 'Hardware Codec', children: [
        _OptionCheckBox(context, 'Enable hardware codec', 'enable-hwcodec'),
      ]),
    );
  }

  Widget audio(BuildContext context) {
    String getDefault() {
      if (Platform.isWindows) return 'System Sound';
      return '';
    }

    Future<String> getValue() async {
      String device = await bind.mainGetOption(key: 'audio-input');
      if (device.isNotEmpty) {
        return device;
      } else {
        return getDefault();
      }
    }

    setDevice(String device) {
      if (device == getDefault()) device = '';
      bind.mainSetOption(key: 'audio-input', value: device);
    }

    return _futureBuilder(future: () async {
      List<String> devices = (await bind.mainGetSoundInputs()).toList();
      if (Platform.isWindows) {
        devices.insert(0, 'System Sound');
      }
      String current = await getValue();
      return {'devices': devices, 'current': current};
    }(), hasData: (data) {
      String currentDevice = data['current'];
      List<String> devices = data['devices'] as List<String>;
      if (devices.isEmpty) {
        return const Offstage();
      }
      return _Card(title: 'Audio Input Device', children: [
        ...devices.map((device) => _Radio<String>(context,
                value: device,
                groupValue: currentDevice,
                autoNewLine: false,
                label: device, onChanged: (value) {
              setDevice(value);
              setState(() {});
            }))
      ]);
    });
  }

  Widget record(BuildContext context) {
    return _futureBuilder(future: () async {
      String customDirectory =
          await bind.mainGetOption(key: 'video-save-directory');
      String defaultDirectory = await bind.mainDefaultVideoSaveDirectory();
      String dir;
      if (customDirectory.isNotEmpty) {
        dir = customDirectory;
      } else {
        dir = defaultDirectory;
      }
      // canLaunchUrl blocked on windows portable, user SYSTEM
      return {'dir': dir, 'canlaunch': true};
    }(), hasData: (data) {
      Map<String, dynamic> map = data as Map<String, dynamic>;
      String dir = map['dir']!;
      bool canlaunch = map['canlaunch']! as bool;

      return _Card(title: 'Recording', children: [
        _OptionCheckBox(context, 'Automatically record incoming sessions',
            'allow-auto-record-incoming'),
        Row(
          children: [
            Text('${translate("Directory")}:'),
            Expanded(
              child: GestureDetector(
                  onTap: canlaunch ? () => launchUrl(Uri.file(dir)) : null,
                  child: Text(
                    dir,
                    softWrap: true,
                    style:
                        const TextStyle(decoration: TextDecoration.underline),
                  )).marginOnly(left: 10),
            ),
            ElevatedButton(
                    onPressed: () async {
                      String? selectedDirectory = await FilePicker.platform
                          .getDirectoryPath(initialDirectory: dir);
                      if (selectedDirectory != null) {
                        await bind.mainSetOption(
                            key: 'video-save-directory',
                            value: selectedDirectory);
                        setState(() {});
                      }
                    },
                    child: Text(translate('Change')))
                .marginOnly(left: 5),
          ],
        ).marginOnly(left: _kContentHMargin),
      ]);
    });
  }

  Widget language() {
    return _futureBuilder(future: () async {
      String langs = await bind.mainGetLangs();
      String lang = bind.mainGetLocalOption(key: kCommConfKeyLang);
      return {'langs': langs, 'lang': lang};
    }(), hasData: (res) {
      Map<String, String> data = res as Map<String, String>;
      List<dynamic> langsList = jsonDecode(data['langs']!);
      Map<String, String> langsMap = {for (var v in langsList) v[0]: v[1]};
      List<String> keys = langsMap.keys.toList();
      List<String> values = langsMap.values.toList();
      keys.insert(0, '');
      values.insert(0, 'Default');
      String currentKey = data['lang']!;
      if (!keys.contains(currentKey)) {
        currentKey = '';
      }
      return _ComboBox(
        keys: keys,
        values: values,
        initialKey: currentKey,
        onChanged: (key) async {
          await bind.mainSetLocalOption(key: kCommConfKeyLang, value: key);
          reloadAllWindows();
          bind.mainChangeLanguage(lang: key);
        },
      ).marginOnly(left: _kContentHMargin);
    });
  }
}

enum _AccessMode {
  custom,
  full,
  view,
  deny,
}

class _Safety extends StatefulWidget {
  const _Safety({Key? key}) : super(key: key);

  @override
  State<_Safety> createState() => _SafetyState();
}

class _SafetyState extends State<_Safety> with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;
  bool locked = bind.mainIsInstalled();
  final scrollController = ScrollController();
  final RxBool serviceStop = Get.find<RxBool>(tag: 'stop-service');

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return DesktopScrollWrapper(
        scrollController: scrollController,
        child: SingleChildScrollView(
            physics: DraggableNeverScrollableScrollPhysics(),
            controller: scrollController,
            child: Column(
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
                    _Card(title: 'ID', children: [changeId()]),
                    more(context),
                  ]),
                ),
              ],
            )).marginOnly(bottom: _kListViewBottomMargin));
  }

  Widget changeId() {
    return _Button('Change ID', changeIdDialog, enabled: !locked);
  }

  Widget permissions(context) {
    return Obx(() => _permissions(context, serviceStop.value));
  }

  Widget _permissions(context, bool stopService) {
    bool enabled = !locked;
    return _futureBuilder(future: () async {
      return await bind.mainGetOption(key: 'access-mode');
    }(), hasData: (data) {
      String accessMode = data! as String;
      _AccessMode mode;
      if (stopService) {
        mode = _AccessMode.deny;
      } else {
        if (accessMode == 'full') {
          mode = _AccessMode.full;
        } else if (accessMode == 'view') {
          mode = _AccessMode.view;
        } else {
          mode = _AccessMode.custom;
        }
      }
      String initialKey;
      bool? fakeValue;
      switch (mode) {
        case _AccessMode.custom:
          initialKey = '';
          fakeValue = null;
          break;
        case _AccessMode.full:
          initialKey = 'full';
          fakeValue = true;
          break;
        case _AccessMode.view:
          initialKey = 'view';
          fakeValue = false;
          break;
        case _AccessMode.deny:
          initialKey = 'deny';
          fakeValue = false;
          break;
      }

      return _Card(title: 'Permissions', children: [
        _ComboBox(
            keys: [
              '',
              'full',
              'view',
              'deny'
            ],
            values: [
              translate('Custom'),
              translate('Full Access'),
              translate('Screen Share'),
              translate('Deny remote access'),
            ],
            initialKey: initialKey,
            onChanged: (mode) async {
              String modeValue;
              bool stopService;
              if (mode == 'deny') {
                modeValue = '';
                stopService = true;
              } else {
                modeValue = mode;
                stopService = false;
              }
              await bind.mainSetOption(key: 'access-mode', value: modeValue);
              await bind.mainSetOption(
                  key: 'stop-service',
                  value: bool2option('stop-service', stopService));
              setState(() {});
            }).marginOnly(left: _kContentHMargin),
        Offstage(
          offstage: mode == _AccessMode.deny,
          child: Column(
            children: [
              _OptionCheckBox(
                  context, 'Enable Keyboard/Mouse', 'enable-keyboard',
                  enabled: enabled, fakeValue: fakeValue),
              _OptionCheckBox(context, 'Enable Clipboard', 'enable-clipboard',
                  enabled: enabled, fakeValue: fakeValue),
              _OptionCheckBox(
                  context, 'Enable File Transfer', 'enable-file-transfer',
                  enabled: enabled, fakeValue: fakeValue),
              _OptionCheckBox(context, 'Enable Audio', 'enable-audio',
                  enabled: enabled, fakeValue: fakeValue),
              _OptionCheckBox(context, 'Enable TCP Tunneling', 'enable-tunnel',
                  enabled: enabled, fakeValue: fakeValue),
              _OptionCheckBox(
                  context, 'Enable Remote Restart', 'enable-remote-restart',
                  enabled: enabled, fakeValue: fakeValue),
              _OptionCheckBox(
                  context, 'Enable Recording Session', 'enable-record-session',
                  enabled: enabled, fakeValue: fakeValue),
              _OptionCheckBox(
                  context,
                  'Enable remote configuration modification',
                  'allow-remote-config-modification',
                  enabled: enabled,
                  fakeValue: fakeValue),
            ],
          ),
        )
      ]);
    });
  }

  Widget password(BuildContext context) {
    return ChangeNotifierProvider.value(
        value: gFFI.serverModel,
        child: Consumer<ServerModel>(builder: ((context, model, child) {
          List<String> passwordKeys = [
            kUseTemporaryPassword,
            kUsePermanentPassword,
            kUseBothPasswords,
          ];
          List<String> passwordValues = [
            translate('Use one-time password'),
            translate('Use permanent password'),
            translate('Use both passwords'),
          ];
          bool tmpEnabled = model.verificationMethod != kUsePermanentPassword;
          bool permEnabled = model.verificationMethod != kUseTemporaryPassword;
          String currentValue =
              passwordValues[passwordKeys.indexOf(model.verificationMethod)];
          List<Widget> radios = passwordValues
              .map((value) => _Radio<String>(
                    context,
                    value: value,
                    groupValue: currentValue,
                    label: value,
                    onChanged: ((value) {
                      () async {
                        await model.setVerificationMethod(
                            passwordKeys[passwordValues.indexOf(value)]);
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

          final modeKeys = ['password', 'click', ''];
          final modeValues = [
            translate('Accept sessions via password'),
            translate('Accept sessions via click'),
            translate('Accept sessions via both'),
          ];
          var modeInitialKey = model.approveMode;
          if (!modeKeys.contains(modeInitialKey)) modeInitialKey = '';
          final usePassword = model.approveMode != 'click';

          return _Card(title: 'Password', children: [
            _ComboBox(
              keys: modeKeys,
              values: modeValues,
              initialKey: modeInitialKey,
              onChanged: (key) => model.setApproveMode(key),
            ).marginOnly(left: _kContentHMargin),
            if (usePassword) radios[0],
            if (usePassword)
              _SubLabeledWidget(
                  'One-time password length',
                  Row(
                    children: [
                      ...lengthRadios,
                    ],
                  ),
                  enabled: tmpEnabled && !locked),
            if (usePassword) radios[1],
            if (usePassword)
              _SubButton('Set permanent password', setPasswordDialog,
                  permEnabled && !locked),
            if (usePassword)
              hide_cm(!locked).marginOnly(left: _kContentHSubMargin - 6),
            if (usePassword) radios[2],
          ]);
        })));
  }

  Widget more(BuildContext context) {
    bool enabled = !locked;
    return _Card(title: 'Security', children: [
      Offstage(
        offstage: !Platform.isWindows,
        child: _OptionCheckBox(context, 'Enable RDP', 'enable-rdp',
            enabled: enabled),
      ),
      shareRdp(context, enabled),
      _OptionCheckBox(context, 'Deny LAN Discovery', 'enable-lan-discovery',
          reverse: true, enabled: enabled),
      ...directIp(context),
      whitelist(),
    ]);
  }

  shareRdp(BuildContext context, bool enabled) {
    onChanged(bool b) async {
      await bind.mainSetShareRdp(enable: b);
      setState(() {});
    }

    bool value = bind.mainIsShareRdp();
    return Offstage(
      offstage: !(Platform.isWindows && bind.mainIsRdpServiceOpen()),
      child: GestureDetector(
          child: Row(
            children: [
              Checkbox(
                      value: value,
                      onChanged: enabled ? (_) => onChanged(!value) : null)
                  .marginOnly(right: 5),
              Expanded(
                child: Text(translate('Enable RDP session sharing'),
                    style:
                        TextStyle(color: _disabledTextColor(context, enabled))),
              )
            ],
          ).marginOnly(left: _kCheckBoxLeftMargin),
          onTap: enabled ? () => onChanged(!value) : null),
    );
  }

  List<Widget> directIp(BuildContext context) {
    TextEditingController controller = TextEditingController();
    update() => setState(() {});
    RxBool applyEnabled = false.obs;
    return [
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
          if (!enabled) applyEnabled.value = false;
          controller.text = data['port'].toString();
          return Offstage(
            offstage: !enabled,
            child: Row(children: [
              _SubLabeledWidget(
                'Port',
                SizedBox(
                  width: 80,
                  child: TextField(
                    controller: controller,
                    enabled: enabled && !locked,
                    onChanged: (_) => applyEnabled.value = true,
                    inputFormatters: [
                      FilteringTextInputFormatter.allow(RegExp(
                          r'^([0-9]|[1-9]\d|[1-9]\d{2}|[1-9]\d{3}|[1-5]\d{4}|6[0-4]\d{3}|65[0-4]\d{2}|655[0-2]\d|6553[0-5])$')),
                    ],
                    textAlign: TextAlign.end,
                    decoration: const InputDecoration(
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
                    onPressed: applyEnabled.value && enabled && !locked
                        ? () async {
                            applyEnabled.value = false;
                            await bind.mainSetOption(
                                key: 'direct-access-port',
                                value: controller.text);
                          }
                        : null,
                    child: Text(
                      translate('Apply'),
                    ),
                  ).marginOnly(left: 20))
            ]),
          );
        },
      ),
    ];
  }

  Widget whitelist() {
    bool enabled = !locked;
    return _futureBuilder(future: () async {
      return await bind.mainGetOption(key: 'whitelist');
    }(), hasData: (data) {
      RxBool hasWhitelist = (data as String).isNotEmpty.obs;
      update() async {
        hasWhitelist.value =
            (await bind.mainGetOption(key: 'whitelist')).isNotEmpty;
      }

      onChanged(bool? checked) async {
        changeWhiteList(callback: update);
      }

      return GestureDetector(
        child: Tooltip(
          message: translate('whitelist_tip'),
          child: Obx(() => Row(
                children: [
                  Checkbox(
                          value: hasWhitelist.value,
                          onChanged: enabled ? onChanged : null)
                      .marginOnly(right: 5),
                  Offstage(
                    offstage: !hasWhitelist.value,
                    child: const Icon(Icons.warning_amber_rounded,
                            color: Color.fromARGB(255, 255, 204, 0))
                        .marginOnly(right: 5),
                  ),
                  Expanded(
                      child: Text(
                    translate('Use IP Whitelisting'),
                    style:
                        TextStyle(color: _disabledTextColor(context, enabled)),
                  ))
                ],
              )),
        ),
        onTap: () {
          onChanged(!hasWhitelist.value);
        },
      ).marginOnly(left: _kCheckBoxLeftMargin);
    });
  }

  Widget hide_cm(bool enabled) {
    return ChangeNotifierProvider.value(
        value: gFFI.serverModel,
        child: Consumer<ServerModel>(builder: (context, model, child) {
          final enableHideCm = model.approveMode == 'password' &&
              model.verificationMethod == kUsePermanentPassword;
          onHideCmChanged(bool? b) {
            if (b != null) {
              bind.mainSetOption(
                  key: 'allow-hide-cm', value: bool2option('allow-hide-cm', b));
            }
          }

          return Tooltip(
              message: enableHideCm ? "" : translate('hide_cm_tip'),
              child: GestureDetector(
                onTap:
                    enableHideCm ? () => onHideCmChanged(!model.hideCm) : null,
                child: Row(
                  children: [
                    Checkbox(
                            value: model.hideCm,
                            onChanged: enabled && enableHideCm
                                ? onHideCmChanged
                                : null)
                        .marginOnly(right: 5),
                    Expanded(
                      child: Text(
                        translate('Hide connection management window'),
                        style: TextStyle(
                            color: _disabledTextColor(
                                context, enabled && enableHideCm)),
                      ),
                    ),
                  ],
                ),
              ));
        }));
  }
}

class _Network extends StatefulWidget {
  const _Network({Key? key}) : super(key: key);

  @override
  State<_Network> createState() => _NetworkState();
}

class _NetworkState extends State<_Network> with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;
  bool locked = bind.mainIsInstalled();

  @override
  Widget build(BuildContext context) {
    super.build(context);
    bool enabled = !locked;
    final scrollController = ScrollController();
    return DesktopScrollWrapper(
        scrollController: scrollController,
        child: ListView(
            controller: scrollController,
            physics: DraggableNeverScrollableScrollPhysics(),
            children: [
              _lock(locked, 'Unlock Network Settings', () {
                locked = false;
                setState(() => {});
              }),
              AbsorbPointer(
                absorbing: locked,
                child: Column(children: [
                  server(enabled),
                  _Card(title: 'Proxy', children: [
                    _Button('Socks5 Proxy', changeSocks5Proxy,
                        enabled: enabled),
                  ]),
                ]),
              ),
            ]).marginOnly(bottom: _kListViewBottomMargin));
  }

  server(bool enabled) {
    return _futureBuilder(future: () async {
      return await bind.mainGetOptions();
    }(), hasData: (data) {
      // Setting page is not modal, oldOptions should only be used when getting options, never when setting.
      Map<String, dynamic> oldOptions = jsonDecode(data! as String);
      old(String key) {
        return (oldOptions[key] ?? '').trim();
      }

      RxString idErrMsg = ''.obs;
      RxString relayErrMsg = ''.obs;
      RxString apiErrMsg = ''.obs;
      var idController =
          TextEditingController(text: old('custom-rendezvous-server'));
      var relayController = TextEditingController(text: old('relay-server'));
      var apiController = TextEditingController(text: old('api-server'));
      var keyController = TextEditingController(text: old('key'));

      set(String idServer, String relayServer, String apiServer,
          String key) async {
        idServer = idServer.trim();
        relayServer = relayServer.trim();
        apiServer = apiServer.trim();
        key = key.trim();
        if (idServer.isNotEmpty) {
          idErrMsg.value =
              translate(await bind.mainTestIfValidServer(server: idServer));
          if (idErrMsg.isNotEmpty) {
            return false;
          }
        }
        if (relayServer.isNotEmpty) {
          relayErrMsg.value =
              translate(await bind.mainTestIfValidServer(server: relayServer));
          if (relayErrMsg.isNotEmpty) {
            return false;
          }
        }
        if (apiServer.isNotEmpty) {
          if (!apiServer.startsWith('http://') &&
              !apiServer.startsWith('https://')) {
            apiErrMsg.value =
                '${translate("API Server")}: ${translate("invalid_http")}';
            return false;
          }
        }
        final old = await bind.mainGetOption(key: 'custom-rendezvous-server');
        if (old.isNotEmpty && old != idServer) {
          await gFFI.userModel.logOut();
        }
        // should set one by one
        await bind.mainSetOption(
            key: 'custom-rendezvous-server', value: idServer);
        await bind.mainSetOption(key: 'relay-server', value: relayServer);
        await bind.mainSetOption(key: 'api-server', value: apiServer);
        await bind.mainSetOption(key: 'key', value: key);
        return true;
      }

      submit() async {
        bool result = await set(idController.text, relayController.text,
            apiController.text, keyController.text);
        if (result) {
          setState(() {});
          showToast(translate('Successful'));
        } else {
          showToast(translate('Failed'));
        }
      }

      import() {
        Clipboard.getData(Clipboard.kTextPlain).then((value) {
          final text = value?.text;
          if (text != null && text.isNotEmpty) {
            try {
              final sc = ServerConfig.decode(text);
              if (sc.idServer.isNotEmpty) {
                idController.text = sc.idServer;
                relayController.text = sc.relayServer;
                apiController.text = sc.apiServer;
                keyController.text = sc.key;
                Future<bool> success =
                    set(sc.idServer, sc.relayServer, sc.apiServer, sc.key);
                success.then((value) {
                  if (value) {
                    showToast(
                        translate('Import server configuration successfully'));
                  } else {
                    showToast(translate('Invalid server configuration'));
                  }
                });
              } else {
                showToast(translate('Invalid server configuration'));
              }
            } catch (e) {
              showToast(translate('Invalid server configuration'));
            }
          } else {
            showToast(translate('Clipboard is empty'));
          }
        });
      }

      export() {
        final text = ServerConfig(
                idServer: idController.text,
                relayServer: relayController.text,
                apiServer: apiController.text,
                key: keyController.text)
            .encode();
        debugPrint("ServerConfig export: $text");

        Clipboard.setData(ClipboardData(text: text));
        showToast(translate('Export server configuration successfully'));
      }

      bool secure = !enabled;
      return _Card(title: 'ID/Relay Server', title_suffix: [
        Tooltip(
          message: translate('Import Server Config'),
          child: IconButton(
              icon: Icon(Icons.paste, color: Colors.grey),
              onPressed: enabled ? import : null),
        ),
        Tooltip(
            message: translate('Export Server Config'),
            child: IconButton(
                icon: Icon(Icons.copy, color: Colors.grey),
                onPressed: enabled ? export : null)),
      ], children: [
        Column(
          children: [
            Obx(() => _LabeledTextField(context, 'ID Server', idController,
                idErrMsg.value, enabled, secure)),
            Obx(() => _LabeledTextField(context, 'Relay Server',
                relayController, relayErrMsg.value, enabled, secure)),
            Obx(() => _LabeledTextField(context, 'API Server', apiController,
                apiErrMsg.value, enabled, secure)),
            _LabeledTextField(
                context, 'Key', keyController, '', enabled, secure),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [_Button('Apply', submit, enabled: enabled)],
            ).marginOnly(top: 15),
          ],
        )
      ]);
    });
  }
}

class _Display extends StatefulWidget {
  const _Display({Key? key}) : super(key: key);

  @override
  State<_Display> createState() => _DisplayState();
}

class _DisplayState extends State<_Display> {
  @override
  Widget build(BuildContext context) {
    final scrollController = ScrollController();
    return DesktopScrollWrapper(
        scrollController: scrollController,
        child: ListView(
            controller: scrollController,
            physics: DraggableNeverScrollableScrollPhysics(),
            children: [
              viewStyle(context),
              scrollStyle(context),
              imageQuality(context),
              codec(context),
              other(context),
            ]).marginOnly(bottom: _kListViewBottomMargin));
  }

  Widget viewStyle(BuildContext context) {
    final key = 'view_style';
    onChanged(String value) async {
      await bind.mainSetUserDefaultOption(key: key, value: value);
      setState(() {});
    }

    final groupValue = bind.mainGetUserDefaultOption(key: key);
    return _Card(title: 'Default View Style', children: [
      _Radio(context,
          value: kRemoteViewStyleOriginal,
          groupValue: groupValue,
          label: 'Scale original',
          onChanged: onChanged),
      _Radio(context,
          value: kRemoteViewStyleAdaptive,
          groupValue: groupValue,
          label: 'Scale adaptive',
          onChanged: onChanged),
    ]);
  }

  Widget scrollStyle(BuildContext context) {
    final key = 'scroll_style';
    onChanged(String value) async {
      await bind.mainSetUserDefaultOption(key: key, value: value);
      setState(() {});
    }

    final groupValue = bind.mainGetUserDefaultOption(key: key);
    return _Card(title: 'Default Scroll Style', children: [
      _Radio(context,
          value: kRemoteScrollStyleAuto,
          groupValue: groupValue,
          label: 'ScrollAuto',
          onChanged: onChanged),
      _Radio(context,
          value: kRemoteScrollStyleBar,
          groupValue: groupValue,
          label: 'Scrollbar',
          onChanged: onChanged),
    ]);
  }

  Widget imageQuality(BuildContext context) {
    final key = 'image_quality';
    onChanged(String value) async {
      await bind.mainSetUserDefaultOption(key: key, value: value);
      setState(() {});
    }

    final groupValue = bind.mainGetUserDefaultOption(key: key);
    final qualityKey = 'custom_image_quality';
    final qualityValue =
        (double.tryParse(bind.mainGetUserDefaultOption(key: qualityKey)) ??
                50.0)
            .obs;
    final fpsKey = 'custom-fps';
    final fpsValue =
        (double.tryParse(bind.mainGetUserDefaultOption(key: fpsKey)) ?? 30.0)
            .obs;
    return _Card(title: 'Default Image Quality', children: [
      _Radio(context,
          value: kRemoteImageQualityBest,
          groupValue: groupValue,
          label: 'Good image quality',
          onChanged: onChanged),
      _Radio(context,
          value: kRemoteImageQualityBalanced,
          groupValue: groupValue,
          label: 'Balanced',
          onChanged: onChanged),
      _Radio(context,
          value: kRemoteImageQualityLow,
          groupValue: groupValue,
          label: 'Optimize reaction time',
          onChanged: onChanged),
      _Radio(context,
          value: kRemoteImageQualityCustom,
          groupValue: groupValue,
          label: 'Custom',
          onChanged: onChanged),
      Offstage(
        offstage: groupValue != kRemoteImageQualityCustom,
        child: Column(
          children: [
            Obx(() => Row(
                  children: [
                    Slider(
                      value: qualityValue.value,
                      min: 10.0,
                      max: 100.0,
                      divisions: 18,
                      onChanged: (double value) async {
                        qualityValue.value = value;
                        await bind.mainSetUserDefaultOption(
                            key: qualityKey, value: value.toString());
                      },
                    ),
                    SizedBox(
                        width: 40,
                        child: Text(
                          '${qualityValue.value.round()}%',
                          style: const TextStyle(fontSize: 15),
                        )),
                    SizedBox(
                        width: 50,
                        child: Text(
                          translate('Bitrate'),
                          style: const TextStyle(fontSize: 15),
                        ))
                  ],
                )),
            Obx(() => Row(
                  children: [
                    Slider(
                      value: fpsValue.value,
                      min: 10.0,
                      max: 120.0,
                      divisions: 22,
                      onChanged: (double value) async {
                        fpsValue.value = value;
                        await bind.mainSetUserDefaultOption(
                            key: fpsKey, value: value.toString());
                      },
                    ),
                    SizedBox(
                        width: 40,
                        child: Text(
                          '${fpsValue.value.round()}',
                          style: const TextStyle(fontSize: 15),
                        )),
                    SizedBox(
                        width: 50,
                        child: Text(
                          translate('FPS'),
                          style: const TextStyle(fontSize: 15),
                        ))
                  ],
                )),
          ],
        ),
      )
    ]);
  }

  Widget codec(BuildContext context) {
    if (!bind.mainHasHwcodec()) {
      return Offstage();
    }
    final key = 'codec-preference';
    onChanged(String value) async {
      await bind.mainSetUserDefaultOption(key: key, value: value);
      setState(() {});
    }

    final groupValue = bind.mainGetUserDefaultOption(key: key);

    return _Card(title: 'Default Codec', children: [
      _Radio(context,
          value: 'auto',
          groupValue: groupValue,
          label: 'Auto',
          onChanged: onChanged),
      _Radio(context,
          value: 'vp9',
          groupValue: groupValue,
          label: 'VP9',
          onChanged: onChanged),
      _Radio(context,
          value: 'h264',
          groupValue: groupValue,
          label: 'H264',
          onChanged: onChanged),
      _Radio(context,
          value: 'h265',
          groupValue: groupValue,
          label: 'H265',
          onChanged: onChanged),
    ]);
  }

  Widget otherRow(String label, String key) {
    final value = bind.mainGetUserDefaultOption(key: key) == 'Y';
    onChanged(bool b) async {
      await bind.mainSetUserDefaultOption(key: key, value: b ? 'Y' : '');
      setState(() {});
    }

    return GestureDetector(
        child: Row(
          children: [
            Checkbox(value: value, onChanged: (_) => onChanged(!value))
                .marginOnly(right: 5),
            Expanded(
              child: Text(translate(label)),
            )
          ],
        ).marginOnly(left: _kCheckBoxLeftMargin),
        onTap: () => onChanged(!value));
  }

  Widget other(BuildContext context) {
    return _Card(title: 'Other Default Options', children: [
      otherRow('Show remote cursor', 'show_remote_cursor'),
      otherRow('Zoom cursor', 'zoom-cursor'),
      otherRow('Show quality monitor', 'show_quality_monitor'),
      otherRow('Mute', 'disable_audio'),
      otherRow('Allow file copy and paste', 'enable_file_transfer'),
      otherRow('Disable clipboard', 'disable_clipboard'),
      otherRow('Lock after session end', 'lock_after_session_end'),
      otherRow('Privacy mode', 'privacy_mode'),
    ]);
  }
}

class _Account extends StatefulWidget {
  const _Account({Key? key}) : super(key: key);

  @override
  State<_Account> createState() => _AccountState();
}

class _AccountState extends State<_Account> {
  @override
  Widget build(BuildContext context) {
    final scrollController = ScrollController();
    return DesktopScrollWrapper(
        scrollController: scrollController,
        child: ListView(
          physics: DraggableNeverScrollableScrollPhysics(),
          controller: scrollController,
          children: [
            _Card(title: 'Account', children: [accountAction()]),
          ],
        ).marginOnly(bottom: _kListViewBottomMargin));
  }

  Widget accountAction() {
    return Obx(() => _Button(
        gFFI.userModel.userName.value.isEmpty ? 'Login' : 'Logout',
        () => {
              gFFI.userModel.userName.value.isEmpty
                  ? loginDialog()
                  : gFFI.userModel.logOut()
            }));
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
      final buildDate = await bind.mainGetBuildDate();
      return {'license': license, 'version': version, 'buildDate': buildDate};
    }(), hasData: (data) {
      final license = data['license'].toString();
      final version = data['version'].toString();
      final buildDate = data['buildDate'].toString();
      const linkStyle = TextStyle(decoration: TextDecoration.underline);
      final scrollController = ScrollController();
      return DesktopScrollWrapper(
          scrollController: scrollController,
          child: SingleChildScrollView(
            controller: scrollController,
            physics: DraggableNeverScrollableScrollPhysics(),
            child: _Card(title: '${translate('About')} RustDesk', children: [
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const SizedBox(
                    height: 8.0,
                  ),
                  SelectionArea(
                      child: Text('${translate('Version')}: $version')
                          .marginSymmetric(vertical: 4.0)),
                  SelectionArea(
                      child: Text('${translate('Build Date')}: $buildDate')
                          .marginSymmetric(vertical: 4.0)),
                  InkWell(
                      onTap: () {
                        launchUrlString('https://rustdesk.com/privacy');
                      },
                      child: Text(
                        translate('Privacy Statement'),
                        style: linkStyle,
                      ).marginSymmetric(vertical: 4.0)),
                  InkWell(
                      onTap: () {
                        launchUrlString('https://rustdesk.com');
                      },
                      child: Text(
                        translate('Website'),
                        style: linkStyle,
                      ).marginSymmetric(vertical: 4.0)),
                  Container(
                    decoration: const BoxDecoration(color: Color(0xFF2c8cff)),
                    padding:
                        const EdgeInsets.symmetric(vertical: 24, horizontal: 8),
                    child: SelectionArea(
                        child: Row(
                      children: [
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                'Copyright © 2022 Purslane Ltd.\n$license',
                                style: const TextStyle(color: Colors.white),
                              ),
                              Text(
                                translate('Slogan_tip'),
                                style: TextStyle(
                                    fontWeight: FontWeight.w800,
                                    color: Colors.white),
                              )
                            ],
                          ),
                        ),
                      ],
                    )),
                  ).marginSymmetric(vertical: 4.0)
                ],
              ).marginOnly(left: _kContentHMargin)
            ]),
          ));
    });
  }
}

//#endregion

//#region components

// ignore: non_constant_identifier_names
Widget _Card(
    {required String title,
    required List<Widget> children,
    List<Widget>? title_suffix}) {
  return Row(
    children: [
      Flexible(
        child: SizedBox(
          width: _kCardFixedWidth,
          child: Card(
            child: Column(
              children: [
                Row(
                  children: [
                    Expanded(
                        child: Text(
                      translate(title),
                      textAlign: TextAlign.start,
                      style: const TextStyle(
                        fontSize: _kTitleFontSize,
                      ),
                    )),
                    ...?title_suffix
                  ],
                ).marginOnly(left: _kContentHMargin, top: 10, bottom: 10),
                ...children
                    .map((e) => e.marginOnly(top: 4, right: _kContentHMargin)),
              ],
            ).marginOnly(bottom: 10),
          ).marginOnly(left: _kCardLeftMargin, top: 15),
        ),
      ),
    ],
  );
}

Color? _disabledTextColor(BuildContext context, bool enabled) {
  return enabled
      ? null
      : Theme.of(context).textTheme.titleLarge?.color?.withOpacity(0.6);
}

// ignore: non_constant_identifier_names
Widget _OptionCheckBox(BuildContext context, String label, String key,
    {Function()? update,
    bool reverse = false,
    bool enabled = true,
    Icon? checkedIcon,
    bool? fakeValue}) {
  return _futureBuilder(
      future: bind.mainGetOption(key: key),
      hasData: (data) {
        bool value = option2bool(key, data.toString());
        if (reverse) value = !value;
        var ref = value.obs;
        onChanged(option) async {
          if (option != null) {
            ref.value = option;
            if (reverse) option = !option;
            String value = bool2option(key, option);
            await bind.mainSetOption(key: key, value: value);
            update?.call();
          }
        }

        if (fakeValue != null) {
          ref.value = fakeValue;
          enabled = false;
        }

        return GestureDetector(
          child: Obx(
            () => Row(
              children: [
                Checkbox(
                        value: ref.value, onChanged: enabled ? onChanged : null)
                    .marginOnly(right: 5),
                Offstage(
                  offstage: !ref.value || checkedIcon == null,
                  child: checkedIcon?.marginOnly(right: 5),
                ),
                Expanded(
                    child: Text(
                  translate(label),
                  style: TextStyle(color: _disabledTextColor(context, enabled)),
                ))
              ],
            ),
          ).marginOnly(left: _kCheckBoxLeftMargin),
          onTap: enabled
              ? () {
                  onChanged(!ref.value);
                }
              : null,
        );
      });
}

// ignore: non_constant_identifier_names
Widget _Radio<T>(BuildContext context,
    {required T value,
    required T groupValue,
    required String label,
    required Function(T value) onChanged,
    bool autoNewLine = true,
    bool enabled = true}) {
  var onChange = enabled
      ? (T? value) {
          if (value != null) {
            onChanged(value);
          }
        }
      : null;
  return GestureDetector(
    child: Row(
      children: [
        Radio<T>(value: value, groupValue: groupValue, onChanged: onChange),
        Expanded(
          child: Text(translate(label),
                  overflow: autoNewLine ? null : TextOverflow.ellipsis,
                  style: TextStyle(
                      fontSize: _kContentFontSize,
                      color: _disabledTextColor(context, enabled)))
              .marginOnly(left: 5),
        ),
      ],
    ).marginOnly(left: _kRadioLeftMargin),
    onTap: () => onChange?.call(value),
  );
}

// ignore: non_constant_identifier_names
Widget _Button(String label, Function() onPressed,
    {bool enabled = true, String? tip}) {
  var button = ElevatedButton(
    onPressed: enabled ? onPressed : null,
    child: Text(
      translate(label),
    ).marginSymmetric(horizontal: 15),
  );
  StatefulWidget child;
  if (tip == null) {
    child = button;
  } else {
    child = Tooltip(message: translate(tip), child: button);
  }
  return Row(children: [
    child,
  ]).marginOnly(left: _kContentHMargin);
}

// ignore: non_constant_identifier_names
Widget _SubButton(String label, Function() onPressed, [bool enabled = true]) {
  return Row(
    children: [
      ElevatedButton(
        onPressed: enabled ? onPressed : null,
        child: Text(
          translate(label),
        ).marginSymmetric(horizontal: 15),
      ),
    ],
  ).marginOnly(left: _kContentHSubMargin);
}

// ignore: non_constant_identifier_names
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
                              ? const Color(0xFFD7D7D7)
                              : const Color(0xFFCBCBCB),
                          width: hover.value && enabled ? 2 : 1)),
                  child: Row(
                    children: [
                      Container(
                        height: 28,
                        color: (hover.value && enabled)
                            ? const Color(0xFFD7D7D7)
                            : const Color(0xFFCBCBCB),
                        alignment: Alignment.center,
                        padding: const EdgeInsets.symmetric(
                            horizontal: 5, vertical: 2),
                        child: Text(
                          '${translate(label)}: ',
                          style: const TextStyle(fontWeight: FontWeight.w300),
                        ),
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
            debugPrint(snapshot.error.toString());
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
          Flexible(
            child: SizedBox(
              width: _kCardFixedWidth,
              child: Card(
                child: ElevatedButton(
                  child: SizedBox(
                      height: 25,
                      child: Row(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            const Icon(
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
          ),
        ],
      ));
}

_LabeledTextField(
    BuildContext context,
    String label,
    TextEditingController controller,
    String errorText,
    bool enabled,
    bool secure) {
  return Row(
    children: [
      Spacer(flex: 1),
      Expanded(
        flex: 4,
        child: Text(
          '${translate(label)}:',
          textAlign: TextAlign.right,
          style: TextStyle(color: _disabledTextColor(context, enabled)),
        ),
      ),
      Spacer(flex: 1),
      Expanded(
        flex: 10,
        child: TextField(
            controller: controller,
            enabled: enabled,
            obscureText: secure,
            decoration: InputDecoration(
                isDense: true,
                contentPadding: EdgeInsets.symmetric(vertical: 15),
                errorText: errorText.isNotEmpty ? errorText : null),
            style: TextStyle(
              color: _disabledTextColor(context, enabled),
            )),
      ),
      Spacer(flex: 1),
    ],
  );
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
    // ignore: unused_element
    this.enabled = true,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    var index = keys.indexOf(initialKey);
    if (index < 0) {
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
            icon: const Icon(
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
                  style: const TextStyle(fontSize: _kContentFontSize),
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

void changeSocks5Proxy() async {
  var socks = await bind.mainGetSocks();

  String proxy = '';
  String proxyMsg = '';
  String username = '';
  String password = '';
  if (socks.length == 3) {
    proxy = socks[0];
    username = socks[1];
    password = socks[2];
  }
  var proxyController = TextEditingController(text: proxy);
  var userController = TextEditingController(text: username);
  var pwdController = TextEditingController(text: password);
  RxBool obscure = true.obs;

  var isInProgress = false;
  gFFI.dialogManager.show((setState, close) {
    submit() async {
      setState(() {
        proxyMsg = '';
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
      title: Text(translate('Socks5 Proxy')),
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
                    child: Text('${translate("Hostname")}:')
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
                    autofocus: true,
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
                    child: Text('${translate("Username")}:')
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
                    child: Text('${translate("Password")}:')
                        .marginOnly(bottom: 16.0)),
                const SizedBox(
                  width: 24.0,
                ),
                Expanded(
                  child: Obx(() => TextField(
                        obscureText: obscure.value,
                        decoration: InputDecoration(
                            border: const OutlineInputBorder(),
                            suffixIcon: IconButton(
                                onPressed: () => obscure.value = !obscure.value,
                                icon: Icon(obscure.value
                                    ? Icons.visibility_off
                                    : Icons.visibility))),
                        controller: pwdController,
                      )),
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
        dialogButton('Cancel', onPressed: close, isOutline: true),
        dialogButton('OK', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

//#endregion
