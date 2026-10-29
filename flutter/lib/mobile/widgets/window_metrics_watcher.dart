import 'package:flutter/widgets.dart';

class WindowMetricsWatcher extends StatefulWidget {
  const WindowMetricsWatcher({
    super.key,
    required this.onChanged,
    required this.child,
    this.onViewportChanged,
    this.refitOnViewportChange = true,
    this.refitOnKeyboardChange = false,
  });

  final VoidCallback onChanged;
  final Widget child;
  final ValueChanged<Size>? onViewportChanged;
  final bool refitOnViewportChange;
  final bool refitOnKeyboardChange;

  @override
  State<WindowMetricsWatcher> createState() => _WindowMetricsWatcherState();
}

class _WindowMetricsWatcherState extends State<WindowMetricsWatcher> {
  Size? _size;
  double? _devicePixelRatio;
  Size? _viewportSize;
  double? _viewInsetsBottom;
  bool _scheduled = false;
  bool _refitScheduled = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final size = MediaQuery.sizeOf(context);
    final devicePixelRatio = MediaQuery.devicePixelRatioOf(context);
    if (_size == size && _devicePixelRatio == devicePixelRatio) return;
    _size = size;
    _devicePixelRatio = devicePixelRatio;
    _scheduleChange();
  }

  void _scheduleChange({bool refit = true}) {
    _refitScheduled |= refit;
    if (_scheduled) return;
    _scheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _scheduled = false;
      if (!mounted) return;
      final refit = _refitScheduled;
      _refitScheduled = false;
      final viewport = _viewportSize;
      if (viewport != null) widget.onViewportChanged?.call(viewport);
      if (refit) widget.onChanged();
    });
  }

  @override
  Widget build(BuildContext context) {
    if (widget.onViewportChanged == null) return widget.child;
    return LayoutBuilder(builder: (context, constraints) {
      final bottom =
          MediaQueryData.fromView(View.of(context)).viewInsets.bottom;
      final keyboardChanged =
          _viewInsetsBottom != null && bottom != _viewInsetsBottom;
      _viewInsetsBottom = bottom;
      final viewport = constraints.biggest;
      if (viewport.isFinite && _viewportSize != viewport) {
        _viewportSize = viewport;
        _scheduleChange(
            refit: widget.refitOnViewportChange &&
                (!keyboardChanged || widget.refitOnKeyboardChange));
      }
      return widget.child;
    });
  }
}
