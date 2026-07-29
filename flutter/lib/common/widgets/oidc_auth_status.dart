import 'package:flutter/material.dart';

const _statusFontSize = 12.0;
const _statusSpacing = 4.0;
const _desktopActionSize = 28.0;
const _copyIconSize = 16.0;
const _touchPlatforms = <TargetPlatform>{
  TargetPlatform.android,
  TargetPlatform.iOS,
  TargetPlatform.fuchsia,
};

class OidcAuthStatus extends StatelessWidget {
  final String message;
  final String browserFallbackPrompt;
  final String openLabel;
  final String copyTooltip;
  final VoidCallback? onOpen;
  final VoidCallback? onCopy;

  const OidcAuthStatus({
    super.key,
    required this.message,
    required this.browserFallbackPrompt,
    required this.openLabel,
    required this.copyTooltip,
    this.onOpen,
    this.onCopy,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final messageStyle =
        DefaultTextStyle.of(context).style.copyWith(fontSize: _statusFontSize);
    final helperStyle = messageStyle.copyWith(
      color: theme.colorScheme.onSurfaceVariant,
    );
    final linkColor = theme.brightness == Brightness.dark
        ? Colors.blue.shade300
        : Colors.blue.shade800;
    final isTouchPlatform = _touchPlatforms.contains(theme.platform);
    final actionSize =
        isTouchPlatform ? kMinInteractiveDimension : _desktopActionSize;
    final linkMinimumSize = Size(isTouchPlatform ? actionSize : 0, actionSize);
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        SelectableText(message, style: messageStyle),
        if (onOpen != null || onCopy != null)
          Padding(
            padding: const EdgeInsets.only(top: _statusSpacing),
            child: Wrap(
              alignment: WrapAlignment.center,
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: _statusSpacing,
              runSpacing: _statusSpacing,
              children: [
                Text(browserFallbackPrompt, style: helperStyle),
                if (onOpen != null)
                  TextButton(
                    style: TextButton.styleFrom(
                      foregroundColor: linkColor,
                      minimumSize: linkMinimumSize,
                      padding: EdgeInsets.zero,
                      tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                      visualDensity: VisualDensity.standard,
                    ),
                    onPressed: onOpen,
                    child: Text(
                      openLabel,
                      style: const TextStyle(
                        fontSize: _statusFontSize,
                        decoration: TextDecoration.underline,
                      ),
                    ),
                  ),
                if (onCopy != null)
                  IconButton(
                    tooltip: copyTooltip,
                    constraints: BoxConstraints.tightFor(
                      width: actionSize,
                      height: actionSize,
                    ),
                    padding: EdgeInsets.zero,
                    visualDensity: VisualDensity.standard,
                    color: linkColor,
                    iconSize: _copyIconSize,
                    onPressed: onCopy,
                    icon: const Icon(Icons.copy_outlined),
                  ),
              ],
            ),
          ),
      ],
    );
  }
}
