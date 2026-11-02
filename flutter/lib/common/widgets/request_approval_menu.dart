import 'package:flutter/material.dart';

import '../../common.dart';
import '../../desktop/widgets/material_mod_popup_menu.dart' as mod_menu;
import '../../desktop/widgets/popup_menu.dart';

class RequestApprovalMenu extends MenuEntryBase<String> {
  final String title;
  final List<MenuEntryButton<String>> entries;
  final EdgeInsets? padding;
  final double arrowScale;

  RequestApprovalMenu(
      {required this.title,
      required this.entries,
      this.padding,
      this.arrowScale = 1.0});

  @override
  List<mod_menu.PopupMenuEntry<String>> build(
      BuildContext context, MenuConfig conf) {
    final theme = Theme.of(context);
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
      arrowScale: arrowScale,
      barrierColor: isMobile ? Colors.black26 : null,
      padding: (padding ?? EdgeInsets.zero)
          .copyWith(right: (padding?.right ?? 0) + arrowInset),
      entries: [
        if (isMobile)
          _RequestApprovalMenuTitle(title, padding),
        ...entries.map((entry) => MenuEntryButton<String>(
                childBuilder: entry.childBuilder,
                proc: entry.proc,
                padding: entry.padding,
                enabled: entry.enabled,
                dismissOnClicked: true,
                dismissCallback: () {
                  if (Navigator.canPop(context)) Navigator.pop(context);
                },
              )),
      ],
    ).build(context, conf);
  }
}

class _RequestApprovalMenuTitle extends MenuEntryBase<String> {
  final String title;
  final EdgeInsets? padding;

  _RequestApprovalMenuTitle(this.title, this.padding);

  @override
  List<mod_menu.PopupMenuEntry<String>> build(
      BuildContext context, MenuConfig conf) => [
        mod_menu.PopupMenuItem<String>(
          enabled: false,
          height: conf.height,
          padding: EdgeInsets.zero,
          child: TextButton(
            onPressed: null,
            child: Container(
              alignment: AlignmentDirectional.centerStart,
              height: conf.height,
              padding: padding,
              child: Semantics(
                header: true,
                child: Text(title,
                    style: enabledStyle(context)
                        .copyWith(fontWeight: FontWeight.w600)),
              ),
            ),
          ),
        ),
      ];
}
