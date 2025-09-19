import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/pages/desktop_setting_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';
import 'package:flutter_hbb/common/widgets/login.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:flutter_hbb/desktop/widgets/material_mod_popup_menu.dart'
    as mod_menu;
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
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

  void _showAccountMenu(BuildContext context, TapDownDetails details) {
    final offset = details.globalPosition;
    final x = offset.dx;
    final y = max(offset.dy, kDesktopRemoteTabBarHeight + 2);
    final menuPos = RelativeRect.fromLTRB(x, y, x, y);

    final items = <MenuEntryBase<String>>[];

    if (!gFFI.userModel.isLogin) {
      items.addAll([
        _buildMenuItem(
          context,
          icon: Icons.login,
          text: translate('Login'),
          onTap: () => loginDialog(),
        ),
        _buildMenuItem(
          context,
          icon: Icons.person_add,
          text: translate('Register'),
          onTap: () => _console(),
        ),
      ]);
    } else {
      items.addAll([
        _buildMenuItem(
          context,
          icon: Icons.person,
          text: gFFI.userModel.userName.value,
          onTap: () {},
          enabled: false,
        ),
        _buildMenuItem(
          context,
          icon: Icons.logout,
          text: translate('Logout'),
          onTap: () => logOutConfirmDialog(),
        ),
        MenuEntryDivider<String>(),
        _buildMenuItem(
          context,
          icon: Icons.manage_accounts,
          text: translate('Web Console'),
          onTap: () => _console(),
        ),
      ]);
    }

    mod_menu.showMenu(
      context: context,
      position: menuPos,
      items: items
          .map((e) => e.build(
              context,
              const MenuConfig(
                  commonColor: CustomPopupMenuTheme.commonColor,
                  height: CustomPopupMenuTheme.height,
                  dividerHeight: 8)))
          .expand((i) => i)
          .toList(),
      elevation: 8,
    );
  }

  MenuEntryButton<String> _buildMenuItem(
    BuildContext context, {
    required IconData icon,
    required String text,
    required VoidCallback onTap,
    bool enabled = true,
  }) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Row(
        children: [
          Icon(icon),
          SizedBox(width: 8),
          Text(text, style: style),
        ],
      ),
      proc: onTap,
      enabled: enabled ? null : false.obs,
      dismissOnClicked: true,
    );
  }

  void _console() async {
    try {
      final apiServer = await bind.mainGetApiServer();
      final url = Uri.parse(apiServer);
      if (await canLaunchUrl(url)) {
        await launchUrl(url, mode: LaunchMode.externalApplication);
      } else {
        debugPrint('Failed to open browser: $url');
      }
    } catch (e) {
      debugPrint('Failed to open browser: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    final tabWidget = Container(
        child: Scaffold(
            backgroundColor: Theme.of(context).colorScheme.background,
            body: DesktopTab(
              controller: tabController,
              tail: Row(
                children: [
                  if (!(bind.isIncomingOnly() || bind.isDisableAccount()))
                    ActionIcon(
                      message: 'Account',
                      icon: Icons.account_circle_rounded,
                      onTapDown: (details) {
                        _showAccountMenu(context, details);
                      },
                      isClose: false,
                    ),
                  Offstage(
                    offstage: bind.isIncomingOnly() || bind.isDisableSettings(),
                    child: ActionIcon(
                      message: 'Settings',
                      icon: IconFont.menu,
                      onTap: DesktopTabPage.onAddSetting,
                      isClose: false,
                    ),
                  ),
                ],
              ),
            )));
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
