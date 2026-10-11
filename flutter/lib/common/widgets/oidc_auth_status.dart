import 'package:flutter/material.dart';

const _statusFontSize = 12.0;
const _statusSpacing = 4.0;
const _messageActionSpacing = 8.0;
const _desktopActionSize = 28.0;
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
    final urlStyle =
        DefaultTextStyle.of(context).style.copyWith(fontSize: _statusFontSize);
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          widget.browserFallbackPrompt,
          style: helperStyle,
          textAlign: TextAlign.center,
        ),
        Padding(
          padding: const EdgeInsets.only(top: _statusSpacing),
          child: _buildUrl(urlStyle, linkColor, actionSize),
        ),
      ],
    );
  }

  void _copyAndExpand() {
    setState(() => _expanded = true);
    widget.onCopy?.call();
  }

  Widget _buildUrl(TextStyle urlStyle, Color linkColor, double actionSize) {
    final collapsedUrl = SizedBox(
      width: double.infinity,
      child: TextButton(
        style: TextButton.styleFrom(
          foregroundColor: linkColor,
          minimumSize: Size(0, actionSize),
          padding: EdgeInsets.zero,
          tapTargetSize: MaterialTapTargetSize.shrinkWrap,
          visualDensity: VisualDensity.standard,
        ),
        onPressed: _copyAndExpand,
        child: Text(
          widget.authUrl,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          softWrap: false,
          style: urlStyle.copyWith(
            color: linkColor,
            decoration: TextDecoration.underline,
          ),
        ),
      ),
    );
    final collapsedChild = widget.onCopy == null
        ? collapsedUrl
        : Tooltip(message: widget.copyLabel, child: collapsedUrl);
    return Container(
      width: double.infinity,
      constraints: BoxConstraints(minHeight: actionSize),
      alignment: Alignment.centerLeft,
      padding: const EdgeInsets.symmetric(horizontal: _messageActionSpacing),
      decoration: BoxDecoration(
        border: Border.all(color: Theme.of(context).dividerColor),
        borderRadius: BorderRadius.circular(_statusSpacing),
      ),
      child: _expanded
          ? SelectableText(widget.authUrl, style: urlStyle)
          : collapsedChild,
    );
  }
}
