import 'package:flutter/material.dart';

const _statusFontSize = 12.0;
const _statusSpacing = 4.0;
const _messageActionSpacing = 8.0;
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
  final String authUrl;
  final String copyLabel;
  final VoidCallback? onCopy;

  const OidcAuthStatus({
    super.key,
    required this.message,
    required this.browserFallbackPrompt,
    required this.authUrl,
    required this.copyLabel,
    this.onCopy,
  });

  @override
  Widget build(BuildContext context) {
    final messageStyle =
        DefaultTextStyle.of(context).style.copyWith(fontSize: _statusFontSize);
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        SelectableText(message, style: messageStyle),
        if (authUrl.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: _messageActionSpacing),
            child: _OidcAuthFallback(
              browserFallbackPrompt: browserFallbackPrompt,
              authUrl: authUrl,
              copyLabel: copyLabel,
              onCopy: onCopy,
            ),
          ),
      ],
    );
  }
}

class _OidcAuthFallback extends StatefulWidget {
  final String browserFallbackPrompt;
  final String authUrl;
  final String copyLabel;
  final VoidCallback? onCopy;

  const _OidcAuthFallback({
    required this.browserFallbackPrompt,
    required this.authUrl,
    required this.copyLabel,
    required this.onCopy,
  });

  @override
  State<_OidcAuthFallback> createState() => _OidcAuthFallbackState();
}

class _OidcAuthFallbackState extends State<_OidcAuthFallback> {
  bool _expanded = false;

  @override
  void didUpdateWidget(covariant _OidcAuthFallback oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.authUrl != widget.authUrl) {
      _expanded = false;
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final helperStyle = DefaultTextStyle.of(context).style.copyWith(
          fontSize: _statusFontSize,
          color: theme.colorScheme.onSurfaceVariant,
        );
    final linkColor = theme.brightness == Brightness.dark
        ? Colors.blue.shade300
        : Colors.blue.shade800;
    final isTouchPlatform = _touchPlatforms.contains(theme.platform);
    final actionSize =
        isTouchPlatform ? kMinInteractiveDimension : _desktopActionSize;
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        _buildToggle(helperStyle, linkColor, actionSize),
        if (_expanded)
          Padding(
            padding: const EdgeInsets.only(top: _statusSpacing),
            child: _buildUrl(linkColor, actionSize),
          ),
      ],
    );
  }

  Widget _buildToggle(
      TextStyle helperStyle, Color linkColor, double actionSize) {
    return Semantics(
      expanded: _expanded,
      child: TextButton(
        style: TextButton.styleFrom(
          foregroundColor: linkColor,
          minimumSize: Size(0, actionSize),
          padding: EdgeInsets.zero,
          tapTargetSize: MaterialTapTargetSize.shrinkWrap,
          visualDensity: VisualDensity.standard,
        ),
        onPressed: () => setState(() => _expanded = !_expanded),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Flexible(
              child: Text(
                widget.browserFallbackPrompt,
                style: helperStyle,
                textAlign: TextAlign.center,
              ),
            ),
            const SizedBox(width: _statusSpacing),
            Icon(
              _expanded ? Icons.expand_less : Icons.expand_more,
              size: _copyIconSize,
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildUrl(Color linkColor, double actionSize) {
    final urlStyle =
        DefaultTextStyle.of(context).style.copyWith(fontSize: _statusFontSize);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.only(left: _messageActionSpacing),
      decoration: BoxDecoration(
        border: Border.all(color: Theme.of(context).dividerColor),
        borderRadius: BorderRadius.circular(_statusSpacing),
      ),
      child: Row(
        children: [
          Expanded(child: SelectableText(widget.authUrl, style: urlStyle)),
          if (widget.onCopy != null)
            IconButton(
              tooltip: widget.copyLabel,
              constraints: BoxConstraints.tightFor(
                width: actionSize,
                height: actionSize,
              ),
              padding: EdgeInsets.zero,
              visualDensity: VisualDensity.standard,
              color: linkColor,
              iconSize: _copyIconSize,
              onPressed: widget.onCopy,
              icon: const Icon(Icons.copy_outlined),
            ),
        ],
      ),
    );
  }
}
