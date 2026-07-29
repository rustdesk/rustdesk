import 'package:flutter/material.dart';

const _statusFontSize = 12.0;
const _statusSpacing = 4.0;
const _linkHeight = 28.0;

class OidcAuthStatus extends StatelessWidget {
  final String message;
  final String browserFallbackPrompt;
  final String openLabel;
  final VoidCallback? onOpen;

  const OidcAuthStatus({
    super.key,
    required this.message,
    required this.browserFallbackPrompt,
    required this.openLabel,
    this.onOpen,
  });

  @override
  Widget build(BuildContext context) {
    final messageStyle =
        DefaultTextStyle.of(context).style.copyWith(fontSize: _statusFontSize);
    final helperStyle = messageStyle.copyWith(
      color: Theme.of(context).colorScheme.onSurfaceVariant,
    );
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        SelectableText(message, style: messageStyle),
        if (onOpen != null)
          Padding(
            padding: const EdgeInsets.only(top: _statusSpacing),
            child: Wrap(
              alignment: WrapAlignment.center,
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: _statusSpacing,
              runSpacing: _statusSpacing,
              children: [
                Text(browserFallbackPrompt, style: helperStyle),
                TextButton(
                  style: TextButton.styleFrom(
                    foregroundColor: Colors.blue,
                    minimumSize: const Size(0, _linkHeight),
                    padding: EdgeInsets.zero,
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
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
              ],
            ),
          ),
      ],
    );
  }
}
