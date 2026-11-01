import 'package:flutter/material.dart';

import '../../desktop/widgets/material_mod_popup_menu.dart' as mod_menu;
import '../../desktop/widgets/popup_menu.dart';

class RequestApprovalMenu extends MenuEntryBase<String> {
  final String title;
  final List<MenuEntryButton<String>> entries;
  final EdgeInsets? padding;

  RequestApprovalMenu(
      {required this.title, required this.entries, this.padding});

  @override
  List<mod_menu.PopupMenuEntry<String>> build(
      BuildContext context, MenuConfig conf) {
    final theme = Theme.of(context);
    final buttonPadding =
        (TextButtonTheme.of(context).style?.padding?.resolve({}) ??
                ButtonStyleButton.scaledPadding(
                  theme.useMaterial3
                      ? const EdgeInsets.symmetric(horizontal: 12, vertical: 8)
                      : const EdgeInsets.all(8),
                  const EdgeInsets.symmetric(horizontal: 8),
                  const EdgeInsets.symmetric(horizontal: 4),
                  MediaQuery.textScalerOf(context)
                          .scale(theme.textTheme.labelLarge?.fontSize ?? 14) /
                      14,
                ))
            .resolve(Directionality.of(context));
    final checkboxTheme = CheckboxTheme.of(context);
    final checkboxWidth =
        ((checkboxTheme.materialTapTargetSize ?? theme.materialTapTargetSize) ==
                    MaterialTapTargetSize.padded
                ? kMinInteractiveDimension
                : kMinInteractiveDimension - 8) +
            (checkboxTheme.visualDensity ?? theme.visualDensity)
                .baseSizeAdjustment
                .dx;
    final arrowInset = (checkboxWidth - (IconTheme.of(context).size ?? 24)) / 2;
    return MenuEntrySubMenu<String>(
      text: title,
      dismissParentOnCanceled: true,
      // Normal entries contain two TextButtons; the submenu adds a leading gap.
      padding: (padding ?? EdgeInsets.zero)
          .add(EdgeInsets.only(
              left: buttonPadding.left * 2,
              right: buttonPadding.right * 2 + arrowInset))
          .subtract(
              const EdgeInsetsDirectional.only(start: MenuConfig.midPadding))
          .resolve(Directionality.of(context)),
      entries: entries
          .map((entry) => MenuEntryButton<String>(
                childBuilder: entry.childBuilder,
                proc: entry.proc,
                padding: entry.padding,
                enabled: entry.enabled,
                dismissOnClicked: true,
                dismissCallback: () {
                  if (Navigator.canPop(context)) Navigator.pop(context);
                },
              ))
          .toList(),
    ).build(context, conf);
  }
}
