// Reusable Tefway Remote components.
//
// These are the only building blocks the Tefway surfaces use. They are
// deliberately plain: solid surfaces, one-pixel borders, no gradients, and
// status always carries an icon and a text label in addition to colour.

import 'package:flutter/material.dart';

import 'design.dart';
import 'strings.dart';

/// Semantic status, translated to colour + icon + label together.
enum TefwayStatus { neutral, positive, attention, critical, busy }

class _StatusVisual {
  const _StatusVisual(this.color, this.icon);

  final Color color;
  final IconData icon;
}

_StatusVisual _visual(TefwayStatus status) {
  switch (status) {
    case TefwayStatus.positive:
      return const _StatusVisual(
          TefwayColors.positive, Icons.check_circle_outline_rounded);
    case TefwayStatus.attention:
      return const _StatusVisual(
          TefwayColors.attention, Icons.access_time_rounded);
    case TefwayStatus.critical:
      return const _StatusVisual(
          TefwayColors.critical, Icons.error_outline_rounded);
    case TefwayStatus.busy:
      return const _StatusVisual(TefwayColors.actionBlue, Icons.sync_rounded);
    case TefwayStatus.neutral:
      return const _StatusVisual(
          TefwayColors.textSecondary, Icons.remove_circle_outline_rounded);
  }
}

/// Flat surface with a thin border and a discreet radius.
class TefwaySurfaceCard extends StatelessWidget {
  const TefwaySurfaceCard({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.all(TefwaySpacing.lg),
    this.background = TefwayColors.surface,
    this.borderColor = TefwayColors.border,
    this.radius = TefwayRadius.lg,
  });

  final Widget child;
  final EdgeInsetsGeometry padding;
  final Color background;
  final Color borderColor;
  final double radius;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: padding,
      decoration: BoxDecoration(
        color: background,
        borderRadius: BorderRadius.all(Radius.circular(radius)),
        border: Border.all(color: borderColor, width: 1),
      ),
      child: child,
    );
  }
}

/// The Tefway service mark: the brand blue square with the support headset.
class TefwayBrandMark extends StatelessWidget {
  const TefwayBrandMark({super.key, this.size = 40});

  final double size;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      label: TefwayStrings.productName,
      child: Container(
        width: size,
        height: size,
        decoration: BoxDecoration(
          color: TefwayColors.brandBlue,
          borderRadius: BorderRadius.all(Radius.circular(size * 0.28)),
          border: Border.all(color: TefwayColors.actionBlue, width: 1),
        ),
        child: Icon(
          Icons.headset_mic_rounded,
          size: size * 0.56,
          color: TefwayColors.textPrimary,
        ),
      ),
    );
  }
}

/// Product header: mark, product name, and the role this build plays.
class TefwayBrandHeader extends StatelessWidget {
  const TefwayBrandHeader({
    super.key,
    required this.subtitle,
    this.trailing,
    this.markSize = 40,
    this.titleStyle,
  });

  final String subtitle;
  final Widget? trailing;
  final double markSize;
  final TextStyle? titleStyle;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        TefwayBrandMark(size: markSize),
        const SizedBox(width: TefwaySpacing.md),
        Expanded(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                TefwayStrings.productName,
                style: titleStyle ?? TefwayTextStyles.appTitle,
                overflow: TextOverflow.ellipsis,
              ),
              const SizedBox(height: 2),
              Text(
                subtitle,
                style: TefwayTextStyles.label,
                overflow: TextOverflow.ellipsis,
              ),
            ],
          ),
        ),
        if (trailing != null) ...[
          const SizedBox(width: TefwaySpacing.md),
          trailing!,
        ],
      ],
    );
  }
}

/// Primary action of a screen. Label is mandatory: icon-only buttons are not
/// used for destructive or connection actions.
class TefwayPrimaryButton extends StatelessWidget {
  const TefwayPrimaryButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.icon,
  });

  final String label;
  final VoidCallback? onPressed;
  final IconData? icon;

  @override
  Widget build(BuildContext context) {
    final child = Text(label, style: const TextStyle(fontWeight: FontWeight.w600));
    return ElevatedButton(
      onPressed: onPressed,
      style: ElevatedButton.styleFrom(
        minimumSize: const Size(0, 40),
        padding: const EdgeInsets.symmetric(horizontal: TefwaySpacing.lg),
      ),
      child: icon == null
          ? child
          : Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(icon, size: 16, color: TefwayColors.textPrimary),
                const SizedBox(width: TefwaySpacing.sm),
                child,
              ],
            ),
    );
  }
}

/// Secondary action. Visually separated from the primary one, never filled.
class TefwaySecondaryButton extends StatelessWidget {
  const TefwaySecondaryButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.icon,
  });

  final String label;
  final VoidCallback? onPressed;
  final IconData? icon;

  @override
  Widget build(BuildContext context) {
    return OutlinedButton(
      onPressed: onPressed,
      style: OutlinedButton.styleFrom(
        minimumSize: const Size(0, 36),
        padding: const EdgeInsets.symmetric(horizontal: TefwaySpacing.md),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (icon != null) ...[
            Icon(icon, size: 15, color: TefwayColors.textSecondary),
            const SizedBox(width: TefwaySpacing.sm),
          ],
          Text(label, style: TefwayTextStyles.bodySecondary),
        ],
      ),
    );
  }
}

/// Icon + label status line.
class TefwayStatusIndicator extends StatelessWidget {
  const TefwayStatusIndicator({
    super.key,
    required this.status,
    required this.label,
    this.detail,
    this.compact = false,
  });

  final TefwayStatus status;
  final String label;
  final String? detail;
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final visual = _visual(status);
    return Semantics(
      label: detail == null ? label : '$label. $detail',
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(visual.icon, size: compact ? 14 : 16, color: visual.color),
          const SizedBox(width: TefwaySpacing.sm),
          Flexible(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  label,
                  style: TefwayTextStyles.statusLabel.copyWith(color: visual.color),
                ),
                if (detail != null && !compact) ...[
                  const SizedBox(height: 2),
                  Text(detail!, style: TefwayTextStyles.bodySecondary),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// Compact status badge used in headers and section titles.
class TefwayStatusChip extends StatelessWidget {
  const TefwayStatusChip({
    super.key,
    required this.status,
    required this.label,
  });

  final TefwayStatus status;
  final String label;

  @override
  Widget build(BuildContext context) {
    final visual = _visual(status);
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: TefwaySpacing.md,
        vertical: 6,
      ),
      decoration: BoxDecoration(
        color: TefwayColors.surface,
        borderRadius: TefwayRadius.smAll,
        border: Border.all(color: TefwayColors.border, width: 1),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(visual.icon, size: 13, color: visual.color),
          const SizedBox(width: 6),
          Text(
            label,
            style: TefwayTextStyles.statusLabel.copyWith(color: visual.color),
          ),
        ],
      ),
    );
  }
}

class TefwaySectionTitle extends StatelessWidget {
  const TefwaySectionTitle({super.key, required this.title, this.trailing});

  final String title;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(child: Text(title, style: TefwayTextStyles.sectionTitle)),
        if (trailing != null) trailing!,
      ],
    );
  }
}

/// Identification code with a copy affordance. The code itself is selectable
/// and uses tabular figures.
class TefwayDeviceIdCard extends StatelessWidget {
  const TefwayDeviceIdCard({
    super.key,
    required this.label,
    required this.id,
    required this.onCopy,
    this.hint,
  });

  final String label;
  final String id;
  final VoidCallback onCopy;
  final String? hint;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(label, style: TefwayTextStyles.label, textAlign: TextAlign.center),
        const SizedBox(height: TefwaySpacing.md),
        SelectableText(
          id,
          textAlign: TextAlign.center,
          style: TefwayTextStyles.code,
        ),
        const SizedBox(height: TefwaySpacing.sm),
        TextButton.icon(
          onPressed: onCopy,
          icon: const Icon(Icons.copy_all_outlined,
              size: 15, color: TefwayColors.textSecondary),
          label: Text(TefwayStrings.copyId, style: TefwayTextStyles.bodySecondary),
          style: TextButton.styleFrom(
            minimumSize: const Size(0, 32),
            padding: const EdgeInsets.symmetric(horizontal: TefwaySpacing.md),
          ),
        ),
        if (hint != null) ...[
          const SizedBox(height: TefwaySpacing.xs),
          Text(hint!, textAlign: TextAlign.center, style: TefwayTextStyles.caption),
        ],
      ],
    );
  }
}

/// One session of a Master window. Actions affect only this session.
class TefwaySessionCard extends StatelessWidget {
  const TefwaySessionCard({
    super.key,
    required this.title,
    required this.subtitle,
    required this.status,
    required this.statusLabel,
    required this.onOpen,
    required this.onEnd,
  });

  final String title;
  final String subtitle;
  final TefwayStatus status;
  final String statusLabel;
  final VoidCallback onOpen;
  final VoidCallback onEnd;

  @override
  Widget build(BuildContext context) {
    return TefwaySurfaceCard(
      padding: const EdgeInsets.symmetric(
        horizontal: TefwaySpacing.lg,
        vertical: TefwaySpacing.md,
      ),
      child: Row(
        children: [
          const Icon(Icons.desktop_windows_outlined,
              size: 20, color: TefwayColors.textSecondary),
          const SizedBox(width: TefwaySpacing.md),
          Expanded(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title, style: TefwayTextStyles.sectionTitle),
                const SizedBox(height: 2),
                Text(subtitle, style: TefwayTextStyles.caption),
                const SizedBox(height: TefwaySpacing.sm),
                TefwayStatusIndicator(
                  status: status,
                  label: statusLabel,
                  compact: true,
                ),
              ],
            ),
          ),
          const SizedBox(width: TefwaySpacing.sm),
          Tooltip(
            message: TefwayStrings.openSession,
            child: TextButton(
              onPressed: onOpen,
              style: TextButton.styleFrom(
                minimumSize: const Size(0, 34),
                padding: const EdgeInsets.symmetric(horizontal: TefwaySpacing.md),
              ),
              child: Text('Abrir', style: TefwayTextStyles.bodySecondary),
            ),
          ),
          Tooltip(
            message: TefwayStrings.endSession,
            child: TextButton(
              onPressed: onEnd,
              style: TextButton.styleFrom(
                minimumSize: const Size(0, 34),
                padding: const EdgeInsets.symmetric(horizontal: TefwaySpacing.md),
              ),
              child: const Text(
                'Encerrar',
                style: TextStyle(fontSize: 13, color: TefwayColors.critical),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class TefwayEmptyState extends StatelessWidget {
  const TefwayEmptyState({
    super.key,
    required this.icon,
    required this.title,
    required this.message,
  });

  final IconData icon;
  final String title;
  final String message;

  @override
  Widget build(BuildContext context) {
    return TefwaySurfaceCard(
      background: TefwayColors.inputFill,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(icon, size: 16, color: TefwayColors.textSecondary),
              const SizedBox(width: TefwaySpacing.sm),
              Text(title, style: TefwayTextStyles.sectionTitle),
            ],
          ),
          const SizedBox(height: TefwaySpacing.sm),
          Text(message, style: TefwayTextStyles.bodySecondary),
        ],
      ),
    );
  }
}

/// Contextual warning. Replaces the inherited full-width promotional banners:
/// it is small, dismissible when the condition is advisory, and never hides a
/// real capture failure.
class TefwayNotice extends StatefulWidget {
  const TefwayNotice({
    super.key,
    required this.title,
    required this.message,
    this.details,
    this.status = TefwayStatus.attention,
    this.onRetry,
  });

  final String title;
  final String message;
  final String? details;
  final TefwayStatus status;
  final VoidCallback? onRetry;

  @override
  State<TefwayNotice> createState() => _TefwayNoticeState();
}

class _TefwayNoticeState extends State<TefwayNotice> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final visual = _visual(widget.status);
    return TefwaySurfaceCard(
      background: TefwayColors.inputFill,
      borderColor: visual.color.withValues(alpha: 0.5),
      padding: const EdgeInsets.all(TefwaySpacing.md),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(visual.icon, size: 15, color: visual.color),
              const SizedBox(width: TefwaySpacing.sm),
              Expanded(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      widget.title,
                      style: TefwayTextStyles.sectionTitle.copyWith(fontSize: 13),
                    ),
                    const SizedBox(height: 2),
                    Text(widget.message, style: TefwayTextStyles.bodySecondary),
                  ],
                ),
              ),
            ],
          ),
          if (widget.details != null || widget.onRetry != null)
            Padding(
              padding: const EdgeInsets.only(top: TefwaySpacing.sm),
              child: Wrap(
                spacing: TefwaySpacing.sm,
                runSpacing: TefwaySpacing.xs,
                children: [
                  if (widget.details != null)
                    TextButton(
                      onPressed: () => setState(() => _expanded = !_expanded),
                      style: TextButton.styleFrom(
                        minimumSize: const Size(0, 30),
                        padding:
                            const EdgeInsets.symmetric(horizontal: TefwaySpacing.sm),
                      ),
                      child: Text(
                        _expanded
                            ? TefwayStrings.hideDetails
                            : TefwayStrings.learnMore,
                        style: TefwayTextStyles.bodySecondary,
                      ),
                    ),
                  if (widget.onRetry != null)
                    TextButton(
                      onPressed: widget.onRetry,
                      style: TextButton.styleFrom(
                        minimumSize: const Size(0, 30),
                        padding:
                            const EdgeInsets.symmetric(horizontal: TefwaySpacing.sm),
                      ),
                      child: Text(TefwayStrings.retry,
                          style: TefwayTextStyles.bodySecondary),
                    ),
                ],
              ),
            ),
          if (_expanded && widget.details != null)
            Padding(
              padding: const EdgeInsets.only(top: TefwaySpacing.xs),
              child: Text(widget.details!, style: TefwayTextStyles.caption),
            ),
        ],
      ),
    );
  }
}

class TefwaySupportFooter extends StatelessWidget {
  const TefwaySupportFooter({
    super.key,
    required this.version,
    this.extra,
  });

  final String version;
  final String? extra;

  @override
  Widget build(BuildContext context) {
    final parts = <String>[
      if (version.isNotEmpty) '${TefwayStrings.versionLabel} $version',
      if (extra != null) extra!,
      TefwayStrings.licenseNote,
    ];
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        const Divider(height: 1, color: TefwayColors.border),
        const SizedBox(height: TefwaySpacing.md),
        Wrap(
          alignment: WrapAlignment.center,
          spacing: TefwaySpacing.md,
          runSpacing: TefwaySpacing.xs,
          children: [
            Text(TefwayStrings.supportSignature, style: TefwayTextStyles.caption),
            for (final part in parts)
              Text(part, style: TefwayTextStyles.caption),
          ],
        ),
        const SizedBox(height: TefwaySpacing.xs),
      ],
    );
  }
}

/// Page scaffold shared by the Master and Client surfaces.
class TefwayAppShell extends StatelessWidget {
  const TefwayAppShell({
    super.key,
    required this.child,
    this.maxWidth = 760,
    this.scrollable = true,
    this.padding = const EdgeInsets.all(TefwaySpacing.xl),
  });

  final Widget child;
  final double maxWidth;
  final bool scrollable;
  final EdgeInsetsGeometry padding;

  @override
  Widget build(BuildContext context) {
    final content = ConstrainedBox(
      constraints: BoxConstraints(maxWidth: maxWidth),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [child],
      ),
    );
    return Container(
      color: TefwayColors.deepNavy,
      child: scrollable
          ? SingleChildScrollView(padding: padding, child: content)
          : Padding(padding: padding, child: content),
    );
  }
}
