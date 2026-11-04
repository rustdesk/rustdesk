// Portable-session notices.
//
// Only conditions the running machine really reports reach this list. The
// inherited promotional/installation cards are not used by the Tefway build:
// the product is portable by design, so inviting the customer to install a
// service would be both noise and a change of security posture.

import 'package:flutter/material.dart';

import '../common.dart';
import '../models/platform_model.dart';
import 'design.dart';
import 'strings.dart';
import 'widgets.dart';

/// Advisory and blocking notices that apply to this machine right now.
List<Widget> tefwayPortableNotices() {
  final notices = <Widget>[];

  if (isWindows) {
    notices.add(const TefwayNotice(
      title: TefwayStrings.uacTitle,
      message: TefwayStrings.uacBody,
      details: TefwayStrings.uacDetails,
    ));
  }

  if (isLinux) {
    if (bind.mainCurrentIsWayland()) {
      notices.add(const TefwayNotice(
        title: TefwayStrings.waylandTitle,
        message: TefwayStrings.waylandBody,
        details: TefwayStrings.waylandDetails,
      ));
    } else if (bind.mainIsLoginWayland()) {
      notices.add(const TefwayNotice(
        title: TefwayStrings.loginWaylandTitle,
        message: TefwayStrings.loginWaylandBody,
      ));
    }
    if (bind.isSelinuxEnforcing()) {
      notices.add(const TefwayNotice(
        title: TefwayStrings.selinuxTitle,
        message: TefwayStrings.selinuxBody,
        details: TefwayStrings.selinuxDetails,
      ));
    }
  }

  return notices;
}

/// Builds the notice column, spacing the entries when there is more than one.
Widget tefwayNoticeColumn(List<Widget> notices) {
  if (notices.isEmpty) return const SizedBox.shrink();
  return Column(
    mainAxisSize: MainAxisSize.min,
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      for (var i = 0; i < notices.length; i++) ...[
        if (i > 0) const SizedBox(height: TefwaySpacing.sm),
        notices[i],
      ],
    ],
  );
}

/// A real connection error reported by the core, shown as-is instead of hidden
/// behind a banner. [message] is already localised by the Rust side.
class TefwayCoreErrorNotice extends StatelessWidget {
  const TefwayCoreErrorNotice({
    super.key,
    required this.message,
    this.onRetry,
  });

  final String message;
  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) {
    return TefwayNotice(
      status: TefwayStatus.critical,
      title: TefwayStrings.errorTitle,
      message: translate(message),
      onRetry: onRetry,
    );
  }
}
