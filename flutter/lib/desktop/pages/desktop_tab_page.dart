import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/pages/connection_page.dart';
import 'package:flutter_hbb/desktop/pages/desktop_setting_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';
// import 'package:flutter/services.dart';

import '../../common/shared_state.dart';

class DesktopTabPage extends StatefulWidget {
  const DesktopTabPage({Key? key}) : super(key: key);

  @override
  State<DesktopTabPage> createState() => _DesktopTabPageState();

  static void onAddSetting(
      {SettingsTabKey initialPage = SettingsTabKey.general}) {
    try {
      DesktopTabController tabController = Get.find<DesktopTabController>();
      tabController.add(TabInfo(
          key: kTabLabelSettingPage,
          label: kTabLabelSettingPage,
          selectedIcon: Icons.build_sharp,
          unselectedIcon: Icons.build_outlined,
          page: DesktopSettingPage(
            key: const ValueKey(kTabLabelSettingPage),
            initialTabkey: initialPage,
          )));
    } catch (e) {
      debugPrintStack(label: '$e');
    }
  }
}

class _DesktopTabPageState extends State<DesktopTabPage> {
  final tabController = DesktopTabController(tabType: DesktopTabType.main);

  _DesktopTabPageState() {
    RemoteCountState.init();
    Get.put<DesktopTabController>(tabController);
    tabController.add(TabInfo(
        key: kTabLabelHomePage,
        label: kTabLabelHomePage,
        selectedIcon: Icons.home_sharp,
        unselectedIcon: Icons.home_outlined,
        closable: false,
        page: DesktopHomePage(
          key: const ValueKey(kTabLabelHomePage),
        )));
    if (bind.isIncomingOnly()) {
      tabController.onSelected = (key) {
        if (key == kTabLabelHomePage) {
          windowManager.setSize(getIncomingOnlyHomeSize());
          setResizable(false);
        } else {
          windowManager.setSize(getIncomingOnlySettingsSize());
          setResizable(true);
        }
      };
    }
  }

  @override
  void initState() {
    super.initState();
    // HardwareKeyboard.instance.addHandler(_handleKeyEvent);
  }

  /*
  bool _handleKeyEvent(KeyEvent event) {
    if (!mouseIn && event is KeyDownEvent) {
      print('key down: ${event.logicalKey}');
      shouldBeBlocked(_block, canBeBlocked);
    }
    return false; // allow it to propagate
  }
  */

  @override
  void dispose() {
    // HardwareKeyboard.instance.removeHandler(_handleKeyEvent);
    Get.delete<DesktopTabController>();

    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final desktopTab = DesktopTab(
      controller: tabController,
      showTabBar: bind.isIncomingOnly(),
      tail: bind.isIncomingOnly()
          ? Offstage(
              offstage: bind.isDisableSettings(),
              child: ActionIcon(
                message: 'Settings',
                icon: IconFont.menu,
                onTap: DesktopTabPage.onAddSetting,
                isClose: false,
              ),
            )
          : null,
    );
    final tabWidget = Container(
      child: Scaffold(
        backgroundColor: Theme.of(context).colorScheme.background,
        body: bind.isIncomingOnly()
            ? desktopTab
            : LayoutBuilder(
                builder: (context, constraints) {
                  final compactNavigation = constraints.maxWidth < 820;
                  return Row(
                    children: [
                      _RigelWorkspaceSidebar(
                        controller: tabController,
                        compact: compactNavigation,
                      ),
                      VerticalDivider(
                        width: 1,
                        color: MyTheme.color(context).divider,
                      ),
                      Expanded(child: desktopTab),
                    ],
                  );
                },
              ),
      ),
    );
    return isMacOS || kUseCompatibleUiMode
        ? tabWidget
        : Obx(
            () => DragToResizeArea(
              resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
              enableResizeEdges: windowManagerEnableResizeEdges,
              child: tabWidget,
            ),
          );
  }
}

class _RigelWorkspaceSidebar extends StatelessWidget {
  final DesktopTabController controller;
  final bool compact;

  const _RigelWorkspaceSidebar({
    required this.controller,
    required this.compact,
  });

  String? get selectedKey {
    final tabs = controller.state.value.tabs;
    final selected = controller.state.value.selected;
    if (selected < 0 || selected >= tabs.length) return null;
    return tabs[selected].key;
  }

  void _select(String key) {
    controller.jumpToByKey(key);
  }

  void _openConnection() {
    _select(kTabLabelHomePage);
    Future.delayed(const Duration(milliseconds: 140), () {
      ConnectionPage.focusRemoteId();
    });
  }

  void _openSettings() {
    final settings = controller.state.value.tabs
        .where((tab) => tab.key == kTabLabelSettingPage);
    final setting = settings.isEmpty ? null : settings.first;
    if (setting != null) {
      _select(kTabLabelSettingPage);
    } else {
      DesktopTabPage.onAddSetting();
    }
  }

  @override
  Widget build(BuildContext context) {
    return Obx(() {
      final tabs = controller.state.value.tabs;
      final sessions = tabs
          .where((tab) =>
              tab.key != kTabLabelHomePage && tab.key != kTabLabelSettingPage)
          .toList();
      final currentKey = selectedKey;
      const foreground = Colors.white;
      return Container(
        width: compact ? 72 : 236,
        color: MyTheme.navy,
        child: SafeArea(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _brandHeader(context),
              Padding(
                padding: EdgeInsets.fromLTRB(
                    compact ? 12 : 14, 0, compact ? 12 : 14, 18),
                child: SizedBox(
                  height: 42,
                  child: Tooltip(
                    message: 'Connect to device',
                    child: ElevatedButton(
                      onPressed: _openConnection,
                      style: ElevatedButton.styleFrom(
                        backgroundColor: Colors.transparent,
                        foregroundColor: Colors.white,
                        elevation: 0,
                        side: BorderSide(color: Colors.white.withOpacity(0.26)),
                        padding:
                            EdgeInsets.symmetric(horizontal: compact ? 0 : 12),
                      ),
                      child: compact
                          ? const Icon(Icons.add_link, size: 18)
                          : const Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                Icon(Icons.add_link, size: 18),
                                SizedBox(width: 8),
                                Text('Connect to device'),
                              ],
                            ),
                    ),
                  ),
                ),
              ),
              _sectionLabel(context, 'WORKSPACE'),
              _navItem(
                context,
                icon: Icons.dashboard_outlined,
                selectedIcon: Icons.dashboard,
                label: 'Overview',
                selected: currentKey == kTabLabelHomePage,
                onTap: () => _select(kTabLabelHomePage),
              ),
              if (!bind.isDisableSettings())
                _navItem(
                  context,
                  icon: Icons.settings_outlined,
                  selectedIcon: Icons.settings,
                  label: 'Settings',
                  selected: currentKey == kTabLabelSettingPage,
                  onTap: _openSettings,
                ),
              if (sessions.isNotEmpty) ...[
                _sectionLabel(context, 'ACTIVE SESSIONS'),
                Expanded(
                  child: ListView.builder(
                    padding: const EdgeInsets.only(top: 2),
                    itemCount: sessions.length,
                    itemBuilder: (context, index) {
                      final tab = sessions[index];
                      return _navItem(
                        context,
                        icon: Icons.desktop_windows_outlined,
                        selectedIcon: Icons.desktop_windows,
                        label: tab.label,
                        selected: currentKey == tab.key,
                        onTap: () => _select(tab.key),
                      );
                    },
                  ),
                ),
              ] else
                const Spacer(),
              Tooltip(
                message: 'Self-hosted workspace',
                child: Padding(
                  padding: EdgeInsets.fromLTRB(
                      compact ? 0 : 18, 12, compact ? 0 : 18, 16),
                  child: Row(
                    mainAxisAlignment: compact
                        ? MainAxisAlignment.center
                        : MainAxisAlignment.start,
                    children: [
                      Icon(Icons.cloud_done_outlined,
                          size: 17, color: MyTheme.coral),
                      if (!compact) ...[
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            'Self-hosted workspace',
                            style: TextStyle(
                              color: foreground.withOpacity(0.62),
                              fontSize: 11,
                            ),
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      );
    });
  }

  Widget _brandHeader(BuildContext context) {
    return Tooltip(
      message: kProductName,
      child: InkWell(
        onTap: () => _select(kTabLabelHomePage),
        borderRadius: BorderRadius.zero,
        child: Container(
          margin:
              EdgeInsets.fromLTRB(compact ? 10 : 14, 14, compact ? 10 : 14, 18),
          padding:
              EdgeInsets.symmetric(horizontal: compact ? 0 : 10, vertical: 10),
          decoration: BoxDecoration(
            border: Border(
              bottom: BorderSide(color: MyTheme.coral.withOpacity(0.80)),
            ),
          ),
          child: Row(
            mainAxisAlignment:
                compact ? MainAxisAlignment.center : MainAxisAlignment.start,
            children: [
              loadIcon(34),
              if (!compact) ...[
                const SizedBox(width: 9),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: const [
                      Text(
                        kProductName,
                        style: TextStyle(
                          color: Colors.white,
                          fontSize: 16,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                      SizedBox(height: 2),
                      Text(
                        'RIGELIS OPERATIONS',
                        style: TextStyle(
                          color: Color(0x99FFFFFF),
                          fontSize: 10,
                          letterSpacing: 1.0,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Widget _sectionLabel(BuildContext context, String label) {
    if (compact) return const SizedBox(height: 8);
    return Padding(
      padding: const EdgeInsets.fromLTRB(18, 4, 18, 7),
      child: Text(
        label,
        style: Theme.of(context).textTheme.bodySmall?.copyWith(
              fontSize: 10,
              letterSpacing: 1.1,
              fontWeight: FontWeight.w700,
              color: Colors.white.withOpacity(0.46),
            ),
      ),
    );
  }

  Widget _navItem(
    BuildContext context, {
    required IconData icon,
    required IconData selectedIcon,
    required String label,
    required bool selected,
    required VoidCallback onTap,
  }) {
    final item = Padding(
      padding: EdgeInsets.symmetric(horizontal: compact ? 8 : 10, vertical: 2),
      child: Material(
        color: selected ? Colors.white.withOpacity(0.08) : Colors.transparent,
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.zero,
          child: Container(
            decoration: BoxDecoration(
              border: Border(
                left: BorderSide(
                  color: selected ? MyTheme.coral : Colors.transparent,
                  width: 3,
                ),
              ),
            ),
            child: Padding(
              padding: EdgeInsets.symmetric(
                  horizontal: compact ? 0 : 9, vertical: 10),
              child: Row(
                mainAxisAlignment: compact
                    ? MainAxisAlignment.center
                    : MainAxisAlignment.start,
                children: [
                  Icon(
                    selected ? selectedIcon : icon,
                    size: 19,
                    color: selected
                        ? MyTheme.coral
                        : Colors.white.withOpacity(0.64),
                  ),
                  if (!compact) ...[
                    const SizedBox(width: 11),
                    Expanded(
                      child: Text(
                        label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: selected
                              ? Colors.white
                              : Colors.white.withOpacity(0.74),
                          fontSize: 13,
                          fontWeight:
                              selected ? FontWeight.w600 : FontWeight.w400,
                        ),
                      ),
                    ),
                    if (selected)
                      Container(
                        width: 5,
                        height: 5,
                        decoration: const BoxDecoration(
                          color: MyTheme.coral,
                          shape: BoxShape.circle,
                        ),
                      ),
                  ],
                ],
              ),
            ),
          ),
        ),
      ),
    );
    return compact ? Tooltip(message: label, child: item) : item;
  }
}
