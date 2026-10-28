import 'package:flutter/material.dart';

import '../../desktop/widgets/material_mod_popup_menu.dart' as mod_menu;
import '../../desktop/widgets/popup_menu.dart';

class LoginMethodMenu extends MenuEntryBase<String> {
  final MenuEntryButton<String> defaultEntry;
  final List<MenuEntryButton<String>> methods;

  LoginMethodMenu({required this.defaultEntry, required this.methods});

  @override
  List<mod_menu.PopupMenuEntry<String>> build(
      BuildContext context, MenuConfig conf) {
    return [
      _LoginMethodMenuItem(
        height: conf.height,
        child: IntrinsicHeight(
            child:
                Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          Expanded(
              child: MenuEntryButton<String>(
            padding: defaultEntry.padding,
            enabled: defaultEntry.enabled,
            dismissOnClicked: true,
            proc: defaultEntry.proc,
            childBuilder: defaultEntry.childBuilder,
          ).build(context, conf).single),
          PopupMenuChildrenItem<String>(
            height: conf.height,
            padding: EdgeInsets.zero,
            enabled: defaultEntry.enabled,
            itemBuilder: (submenuContext) => methods
                .expand((entry) => MenuEntryButton<String>(
                      childBuilder: entry.childBuilder,
                      proc: entry.proc,
                      padding: entry.padding,
                      enabled: entry.enabled,
                      dismissOnClicked: true,
                      dismissCallback: () {
                        if (Navigator.canPop(context)) Navigator.pop(context);
                      },
                    ).build(submenuContext, conf))
                .toList(),
            child: SizedBox(
              width: 56,
              child: Icon(Icons.keyboard_arrow_right, color: conf.commonColor),
            ),
          ),
        ])),
      )
    ];
  }
}

// PopupMenuItem adds a TextButton around its child; these actions need separate hit targets.
class _LoginMethodMenuItem extends mod_menu.PopupMenuEntry<String> {
  const _LoginMethodMenuItem({required this.height, required this.child});

  final Widget child;

  @override
  final double height;

  @override
  bool represents(String? value) => false;

  @override
  State<_LoginMethodMenuItem> createState() => _LoginMethodMenuItemState();
}

class _LoginMethodMenuItemState extends State<_LoginMethodMenuItem> {
  @override
  Widget build(BuildContext context) => widget.child;
}
