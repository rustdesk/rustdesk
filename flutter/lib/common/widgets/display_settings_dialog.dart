import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/display_scale_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/virtual_display_model.dart';
import 'package:flutter_hbb/utils/display_resolution.dart';
import 'package:flutter_hbb/utils/virtual_display.dart';
import 'display_scale.dart';

bool _hasVirtualDisplayNativeMode(FFI ffi) {
  final scales =
      ffi.ffiModel.pi.platformAdditions['virtual_display_native_scale'];
  return ffi.ffiModel.pi.platform == kPeerPlatformMacOS &&
      ffi.ffiModel.isVirtualDisplayResolution &&
      scales is List &&
      scales.contains(2) &&
      ffi.ffiModel.pi.platformAdditions.containsKey(kMacOSVirtualDisplayModes);
}

bool canChangeDisplaySettings(FFI ffi) {
  final model = ffi.ffiModel;
  final display = model.pi.tryGetDisplayIfNotAllDisplay();
  return ffi.connType == ConnType.defaultConn &&
      model.keyboard &&
      !model.viewOnly &&
      display != null &&
      (display.isVirtualDisplayResolution ||
          model.pi.resolutions.isNotEmpty ||
          (!isWeb && model.pi.platformAdditions['display_scale'] == true));
}

class DisplaySettingsTarget {
  final FFI _ffi;
  final PeerInfo _peer;
  final int _index;
  final Future<DisplayScaleState> Function(int, double, String, String)
      _requestScale;
  Display _display;
  String? _confirmedToken;
  DisplayScaleState? _nativeState;
  (int, int)? _pendingResolution;
  bool _closed = false;

  static const _stale = DisplayScaleError(
      'Display settings changed. Reopen the resolution menu and try again.');

  DisplaySettingsTarget(FFI ffi, this._requestScale)
      : _ffi = ffi,
        _peer = ffi.ffiModel.pi,
        _index = ffi.ffiModel.pi.currentDisplay,
        _display = ffi.ffiModel.pi.tryGetDisplayIfNotAllDisplay()!;

  // macOS mode requests use logical pixels. Linux capture scaling is for input
  // coordinates; its resolution requests, like Windows, use output pixels.
  bool get usesLogicalSize => _peer.platform == kPeerPlatformMacOS;

  double get resolutionPixelRatio =>
      usesLogicalSize ? _display.scale : 1;

  (int, int) get resolution => (
        (_display.width / resolutionPixelRatio).round(),
        (_display.height / resolutionPixelRatio).round()
      );

  Display _selected() {
    if (_closed ||
        !canChangeDisplaySettings(_ffi) ||
        !identical(_ffi.ffiModel.pi, _peer) ||
        _peer.currentDisplay != _index) {
      throw _stale;
    }
    return _peer.tryGetDisplayIfNotAllDisplay()!;
  }

  Future<void> _check() async {
    final selected = _selected();
    if (identical(selected, _display)) return;
    final token = _confirmedToken;
    if (token == null) throw _stale;
    // Capture updates must match the last confirmed native snapshot before
    // allowing another write.
    final current = await _requestScale(_index, 0, '', '');
    _validateState(current);
    if (current.token != token || !identical(selected, _selected())) {
      throw _stale;
    }
    _display = selected;
  }

  void _validateState(DisplayScaleState state,
      {bool allowResolutionChange = false}) {
    _selected();
    final previous = _nativeState;
    if (previous != null &&
        (state.identity != previous.identity ||
            (!allowResolutionChange &&
                state.resolution != previous.resolution))) {
      throw _stale;
    }
  }

  Future<DisplayScaleState> requestScale(double percent, String token) async {
    if (_pendingResolution != null || (percent == 0 && _nativeState != null)) {
      if (percent != 0) throw _stale;
      final selected = _selected();
      final state = await _requestScale(_index, 0, '',
          _pendingResolution == null ? '' : _nativeState!.identity);
      _validateState(state,
          allowResolutionChange: _pendingResolution != null);
      if (!identical(selected, _selected())) throw _stale;
      if (_pendingResolution != null &&
          state.resolution != _pendingResolution) {
        throw const DisplayScaleError(
            'Display settings timed out. Refresh and try again.');
      }
      _display = selected;
      _nativeState = state;
      _confirmedToken = state.token;
      _pendingResolution = null;
      return state;
    }
    await _check();
    final selected = _selected();
    if (!identical(_display, selected)) throw _stale;
    final state = await _requestScale(_index, percent, token, '');
    if (percent == 0 && !identical(selected, _selected())) throw _stale;
    _validateState(state);
    _nativeState = state;
    if (percent != 0 && (state.percent - percent).abs() < 0.000001) {
      _confirmedToken = state.token;
    }
    return state;
  }

  Future<DisplayScaleState?> applyResolution(FutureOr<void> Function() apply,
      {(int, int)? resolution}) async {
    await _check();
    if (_pendingResolution != null || !identical(_display, _selected())) {
      throw _stale;
    }
    final confirm = resolution != null && _nativeState != null;
    await apply();
    if (!confirm) return null;
    _pendingResolution = resolution;
    // The legacy resolution command only acknowledges local dispatch. Read the
    // native mode before allowing scaling to use the new mode's capabilities.
    final elapsed = Stopwatch()..start();
    const timeout = Duration(seconds: 10);
    while (true) {
      final remaining = timeout - elapsed.elapsed;
      if (remaining <= Duration.zero) {
        throw const DisplayScaleError(
            'Display settings timed out. Refresh and try again.');
      }
      final selected = _selected();
      DisplayScaleState? state;
      try {
        state = await _requestScale(_index, 0, '', _nativeState!.identity)
            .timeout(remaining);
      } on TimeoutException {
        throw const DisplayScaleError(
            'Display settings timed out. Refresh and try again.');
      } on DisplayScaleError catch (error) {
        if (error.code != 'snapshot_changed') rethrow;
      }
      _selected();
      if (state != null) {
        _validateState(state, allowResolutionChange: true);
        if (state.resolution == resolution &&
            identical(selected, _selected())) {
          _display = selected;
          _nativeState = state;
          _confirmedToken = state.token;
          _pendingResolution = null;
          return state;
        }
      }
      await Future<void>.delayed(const Duration(milliseconds: 100));
    }
  }

  void close() => _closed = true;
}

Future<void> showDisplaySettingsDialog(
  FFI ffi, {
  (int, int)? localResolution,
  void Function(int, int)? onApplied,
}) async {
  if (!canChangeDisplaySettings(ffi)) return;
  final pi = ffi.ffiModel.pi;
  final display = pi.tryGetDisplayIfNotAllDisplay()!;
  final displayIndex = pi.currentDisplay;
  final target = DisplaySettingsTarget(
      ffi,
      (display, percent, token, expectedIdentity) =>
          DisplayScaleRequests.request(
              ffi.sessionId.toString(),
              (requestId) => bind.sessionRequestDisplayScale(
                  sessionId: ffi.sessionId,
                  requestId: requestId,
                  display: display,
                  percent: percent,
                  token: token,
                  expectedIdentity: expectedIdentity)));
  void showError(String message) => msgBox(
      ffi.sessionId,
      'custom-nook-nocancel-hasclose',
      'Resolution',
      message,
      '',
      ffi.dialogManager);

  final hasNativeMode = _hasVirtualDisplayNativeMode(ffi);
  final nativeScale = hasNativeMode && !isWeb;
  final nativeMode = hasNativeMode
      ? nativeVirtualDisplayMode(pi.platformAdditions, displayIndex)
      : null;
  if (hasNativeMode && nativeMode == null) {
    showError(
        'Display settings changed. Reopen the resolution menu and try again.');
    return;
  }
  final isVirtual = display.isVirtualDisplayResolution;
  final supported = pi.resolutions.map((r) => (r.width, r.height)).toList();
  final nativeDimensions = nativeMode == null
      ? null
      : virtualDisplayResolutionDimensions(nativeMode,
          outputPixels: nativeScale);
  final resolution = target.resolution;
  final resolutionPixelRatio = nativeMode != null
      ? (nativeScale ? 1.0 : nativeMode.$3.toDouble())
      : target.resolutionPixelRatio;
  final width = nativeDimensions?.width ?? resolution.$1;
  final height = nativeDimensions?.height ?? resolution.$2;
  final original = (display.originalWidth, display.originalHeight);
  final (int, int, int)? defaultMode = isVirtual
      ? (nativeScale ? (1920, 1080, 1) : null)
      : (supported.contains(original) ? (original.$1, original.$2, 1) : null);
  if (isAndroid) {
    await ffi.invokeMethod('enable_soft_keyboard', true);
  }
  try {
    await ffi.dialogManager.show(
        (setState, close, context) => CustomAlertDialog(
              title: Text(translate('Display Settings')),
              content: SizedBox(
                width: 400,
                child: DisplaySettings(
                  translate: translate,
                  requestScale:
                      !isWeb && pi.platformAdditions['display_scale'] == true
                          ? target.requestScale
                          : null,
                  minDimension: nativeDimensions?.minDimension ??
                      (isVirtual && pi.platform == kPeerPlatformMacOS
                          ? 320
                          : 1),
                  maxDimension: nativeDimensions?.maxDimension ??
                      (isVirtual
                          ? (pi.platform == kPeerPlatformMacOS ? 4096 : 9999)
                          : 2147483647),
                  width: width,
                  height: height,
                  usesLogicalSize: target.usesLogicalSize,
                  allowArbitrarySize: isVirtual,
                  // Preserve the existing Linux Mint text-field freeze workaround.
                  excludeInputSemantics: isLinux,
                  supportedResolutions: supported,
                  defaultResolution: defaultMode,
                  defaultLabel:
                      isVirtual ? 'Default' : 'resolution_original_tip',
                  localResolution: localResolution,
                  outputPixelRatio: resolutionPixelRatio,
                  scales: nativeScale ? const [1, 2] : const [1],
                  initialScale: nativeScale ? nativeMode!.$3 : 1,
                  onCancel: close,
                  onApply: (width, height, scale) async {
                    final confirmed = await target.applyResolution(() async {
                      if (hasNativeMode &&
                          nativeVirtualDisplayMode(
                                  pi.platformAdditions, displayIndex) !=
                              nativeMode) {
                        throw const DisplayScaleError(
                            'Display settings changed. Reopen the resolution menu and try again.');
                      }
                      if (!isVirtual &&
                          !pi.resolutions.any(
                              (r) => r.width == width && r.height == height)) {
                        throw const DisplayScaleError(
                            'Select a resolution supported by the display');
                      }
                      if (nativeScale) {
                        await VirtualDisplayRequests.request(
                            ffi.sessionId.toString(),
                            (requestId) => bind.sessionConfigureVirtualDisplay(
                                sessionId: ffi.sessionId,
                                requestId: requestId,
                                displayId: nativeMode.$4,
                                width: width,
                                height: height,
                                scale: scale),
                            displayId: nativeMode!.$4,
                            width: width,
                            height: height,
                            scale: scale);
                      } else {
                        await bind.sessionChangeResolution(
                            sessionId: ffi.sessionId,
                            display: displayIndex,
                            width: width,
                            height: height);
                      }
                    }, resolution: (width, height));
                    onApplied?.call(width, height);
                    return confirmed;
                  },
                ),
              ),
            ),
        clickMaskDismiss: false,
        backDismiss: false);
  } finally {
    target.close();
    if (isAndroid) {
      await ffi.invokeMethod('enable_soft_keyboard', false);
    }
  }
}

class DisplaySettings extends StatefulWidget {
  final String Function(String) translate;
  final int minDimension;
  final int maxDimension;
  final int width;
  final int height;
  final FutureOr<DisplayScaleState?> Function(int, int, int) onApply;
  final bool usesLogicalSize;
  final bool allowArbitrarySize;
  final bool excludeInputSemantics;
  final (int, int, int)? defaultResolution;
  final String defaultLabel;
  // Local display size is in physical pixels; Web mode requests may use logical pixels.
  final (int, int)? localResolution;
  final double outputPixelRatio;
  // Advertised modes use input dimensions, before applying the render scale.
  final List<(int, int)> supportedResolutions;
  final List<int> scales;
  final int initialScale;
  final VoidCallback onCancel;
  final Future<DisplayScaleState> Function(double percent, String token)?
      requestScale;

  const DisplaySettings({
    super.key,
    required this.translate,
    required this.minDimension,
    required this.maxDimension,
    required this.width,
    required this.height,
    required this.onApply,
    this.usesLogicalSize = false,
    this.allowArbitrarySize = true,
    this.excludeInputSemantics = false,
    this.defaultResolution,
    this.defaultLabel = 'Default',
    this.localResolution,
    this.outputPixelRatio = 1,
    this.supportedResolutions = const [],
    this.scales = const [1],
    this.initialScale = 1,
    required this.onCancel,
    this.requestScale,
  }) : assert(outputPixelRatio > 0);

  @override
  State<DisplaySettings> createState() => _DisplaySettingsState();
}

class _DisplaySettingsState extends State<DisplaySettings> {
  late int _scale =
      widget.scales.contains(widget.initialScale) ? widget.initialScale : 1;

  late final _width = TextEditingController(text: '${widget.width ~/ _scale}');
  late final _height =
      TextEditingController(text: '${widget.height ~/ _scale}');
  late final _initialRatio =
      _ratioForSize(widget.width ~/ _scale, widget.height ~/ _scale);
  late (int, int)? _ratio = _initialRatio;
  final _widthFocus = FocusNode();
  final _ratioFocus = FocusNode();
  final _customFocus = FocusNode();

  DisplayScaleModel? _systemScale;
  bool _applying = false;
  String? _applyError;
  bool _resolutionApplied = false;
  DisplayScaleState? _scaleSnapshot;
  late (int, int, int) _currentMode =
      (widget.width, widget.height, widget.initialScale);

  @override
  void initState() {
    super.initState();
    final request = widget.requestScale;
    if (request != null) {
      _systemScale = DisplayScaleModel(request)..addListener(_scaleChanged);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _systemScale!.refresh();
      });
    }
  }

  void _scaleChanged() {
    if (!mounted) return;
    final state = _systemScale?.current;
    if (state != null &&
        !_systemScale!.busy &&
        !_systemScale!.needsRefresh &&
        (!identical(state, _scaleSnapshot) ||
            (state.resolution.$1, state.resolution.$2, 1) != _currentMode)) {
      final preserveDraft = _applying || _hasEdits;
      if (_scaleSnapshot != null &&
          _scaleSnapshot!.resolution != state.resolution) {
        _resolutionApplied = true;
      }
      _scaleSnapshot = state;
      _currentMode = (state.resolution.$1, state.resolution.$2, 1);
      if (!_hasChanges && _systemScale!.canApply) _applyError = null;
      if (!preserveDraft) {
        _width.text = '${_currentMode.$1}';
        _height.text = '${_currentMode.$2}';
        _ratio = _ratioForSize(_currentMode.$1, _currentMode.$2);
      }
    }
    setState(() {});
  }

  Future<void> _apply() async {
    if (_applying) return;
    final mode = _editedMode;
    final resolutionChanged = _hasChanges;
    setState(() {
      _applying = true;
      _applyError = null;
    });
    final systemScale = _systemScale;
    final confirmResolution = systemScale?.current != null;
    try {
      if (resolutionChanged) {
        final confirmed = await widget.onApply(mode.$1, mode.$2, mode.$3);
        if (!mounted) return;
        if (confirmResolution) {
          if (confirmed == null || confirmed.resolution != (mode.$1, mode.$2)) {
            throw const DisplayScaleError(
                'Display settings changed. Reopen the resolution menu and try again.');
          }
          systemScale!.acceptConfirmedResolution(confirmed);
          if (!systemScale.canApply) return;
        }
      }
      if (systemScale?.changed == true) {
        if (!await systemScale!.apply() || !mounted) return;
      }
      if (mounted) widget.onCancel();
    } catch (error) {
      if (mounted) {
        setState(() => _applyError = error.toString());
        // A failed confirmation can follow a successful native mode switch.
        // Read back the baseline without resubmitting or rolling back the change.
        if (confirmResolution) await systemScale!.refresh();
      }
    } finally {
      if (mounted) setState(() => _applying = false);
    }
  }

  @override
  void dispose() {
    _systemScale?.dispose();
    _width.dispose();
    _height.dispose();
    _widthFocus.dispose();
    _ratioFocus.dispose();
    _customFocus.dispose();
    super.dispose();
  }

  bool _validSize(int width, int height, int scale) =>
      scale > 0 &&
      widget.scales.contains(scale) &&
      width >= widget.minDimension &&
      width <= widget.maxDimension &&
      height >= widget.minDimension &&
      height <= widget.maxDimension &&
      width % scale == 0 &&
      height % scale == 0 &&
      (widget.allowArbitrarySize ||
          widget.supportedResolutions
              .contains((width ~/ scale, height ~/ scale)));

  bool get _valid => _validSize(_editedMode.$1, _editedMode.$2, _scale);

  int get _minDimension => (widget.minDimension / _scale).ceil();
  int get _maxDimension => widget.maxDimension ~/ _scale;

  (int, int, int) get _editedMode => (
        (int.tryParse(_width.text) ?? 0) * _scale,
        (int.tryParse(_height.text) ?? 0) * _scale,
        _scale
      );

  bool get _hasChanges => _editedMode != _currentMode;
  bool get _hasEdits =>
      _hasChanges ||
      _ratio !=
          _ratioForSize(_currentMode.$1 ~/ _currentMode.$3,
              _currentMode.$2 ~/ _currentMode.$3);

  String _qualityLabel(int scale) =>
      scale == 1 ? widget.translate('Standard') : 'HiDPI';

  String _dimensionLabel(int width, int height) {
    final label = '$width × $height';
    return Directionality.of(context) == TextDirection.rtl
        ? '\u2066$label\u2069'
        : label;
  }

  String _modeLabel((int, int, int) mode) => widget.scales.length > 1 &&
          mode.$3 > 0
      ? '${_dimensionLabel(mode.$1 ~/ mode.$3, mode.$2 ~/ mode.$3)} · ${_qualityLabel(mode.$3)}'
      : _dimensionLabel(mode.$1, mode.$2);

  (int, int)? _ratioForSize(int width, int height) {
    final common = displayAspectRatio(width, height);
    if (common != null ||
        widget.allowArbitrarySize ||
        width <= 0 ||
        height <= 0) {
      return common;
    }
    final divisor = width.gcd(height);
    return (width ~/ divisor, height ~/ divisor);
  }

  List<(int, int)> get _supportedModes => widget.supportedResolutions
      .where((size) => _validSize(size.$1 * _scale, size.$2 * _scale, _scale))
      .toSet()
      .toList();

  List<(int, int)> get _ratios {
    if (!widget.allowArbitrarySize) {
      return _supportedModes
          .map((size) => _ratioForSize(size.$1, size.$2))
          .whereType<(int, int)>()
          .toSet()
          .toList();
    }
    final portrait =
        (int.tryParse(_height.text) ?? 0) > (int.tryParse(_width.text) ?? 0);
    return displayAspectRatios
        .map((ratio) => portrait ? (ratio.$2, ratio.$1) : ratio)
        .toList();
  }

  void _selectRatio((int, int) ratio) {
    final width = int.tryParse(_width.text);
    final currentWidth =
        width != null && width > 0 ? width : widget.width ~/ _scale;
    final targetWidth = currentWidth < ratio.$1 ? ratio.$1 : currentWidth;
    final targetHeight =
        linkedDisplayDimension(targetWidth, ratio, widthChanged: true);
    if (targetHeight == null) return;
    final size = fitDisplayResolution(
        localSize: (targetWidth, targetHeight),
        minDimension: _minDimension,
        maxDimension: _maxDimension,
        scale: 1,
        allowArbitrarySize: widget.allowArbitrarySize,
        supported: _supportedModes
            .where((size) => _ratioForSize(size.$1, size.$2) == ratio)
            .toList());
    if (size == null) return;
    setState(() {
      _width.text = '${size.$1}';
      _height.text = '${size.$2}';
      _ratio = ratio;
    });
  }

  void _editDimension(TextEditingController controller) => setState(() {
        final ratio = _ratio;
        if (ratio == null) return;
        final value = int.tryParse(controller.text);
        if (value == null || value <= 0) return;
        final widthChanged = controller == _width;
        final other = widthChanged ? _height : _width;
        if (!widget.allowArbitrarySize) {
          final matches = _supportedModes.where((size) =>
              _ratioForSize(size.$1, size.$2) == ratio &&
              (widthChanged ? size.$1 : size.$2) == value);
          if (matches.isNotEmpty) {
            other.text =
                '${widthChanged ? matches.first.$2 : matches.first.$1}';
            return;
          }
        }
        final linked =
            linkedDisplayDimension(value, ratio, widthChanged: widthChanged);
        if (linked != null) other.text = '$linked';
      });

  Widget _ratioSelector() {
    final ratios = _ratios;
    // Keep the menu above the session's OverlayEntry dialog, with the same lifetime.
    return MenuAnchor(
      childFocusNode: _ratioFocus,
      consumeOutsideTap: true,
      style: const MenuStyle(
        maximumSize: WidgetStatePropertyAll(Size(double.infinity, 300)),
      ),
      menuChildren: [
        MenuItemButton(
          focusNode: _customFocus,
          leadingIcon: SizedBox(
              width: 18,
              child: _ratio == null ? const Icon(Icons.check, size: 18) : null),
          onPressed: () {
            setState(() => _ratio = null);
            _widthFocus.requestFocus();
            _width.selection =
                TextSelection(baseOffset: 0, extentOffset: _width.text.length);
          },
          child: Text(widget.translate('Custom')),
        ),
        for (final ratio in ratios)
          MenuItemButton(
            leadingIcon: SizedBox(
                width: 18,
                child:
                    _ratio == ratio ? const Icon(Icons.check, size: 18) : null),
            onPressed: () => _selectRatio(ratio),
            child: Text('${ratio.$1}:${ratio.$2}'),
          ),
        if (!widget.allowArbitrarySize && _supportedModes.isNotEmpty)
          SubmenuButton(
            menuChildren: [
              for (final size in _supportedModes)
                MenuItemButton(
                  onPressed: () =>
                      _setMode((size.$1 * _scale, size.$2 * _scale, _scale)),
                  child: Text(_dimensionLabel(size.$1, size.$2)),
                ),
            ],
            child: Text(widget.translate('Available resolutions')),
          ),
      ],
      builder: (context, controller, _) => TextButton.icon(
        key: const ValueKey('resolution-aspect-ratio'),
        focusNode: _ratioFocus,
        onPressed: () {
          if (controller.isOpen) {
            controller.close();
          } else {
            controller.open();
            _customFocus.requestFocus();
          }
        },
        icon: const Icon(Icons.arrow_drop_down),
        label: Text(
            _ratio == null
                ? widget.translate('Custom')
                : '${_ratio!.$1}:${_ratio!.$2}',
            semanticsLabel: '${widget.translate('Aspect ratio')}: '
                '${_ratio == null ? widget.translate('Custom') : '${_ratio!.$1}:${_ratio!.$2}'}'),
      ),
    );
  }

  Widget _dimension(TextEditingController controller, String label) => Expanded(
        child: ExcludeSemantics(
          excluding: widget.excludeInputSemantics,
          child: TextField(
            controller: controller,
            focusNode: controller == _width ? _widthFocus : null,
            keyboardType: TextInputType.number,
            inputFormatters: [FilteringTextInputFormatter.digitsOnly],
            maxLength: widget.maxDimension.toString().length,
            decoration: InputDecoration(
                labelText: widget.translate(label),
                suffixText: 'px',
                counterText: ''),
            onChanged: (_) => _editDimension(controller),
          ),
        ),
      );

  (int, int) _localSize(BuildContext context) {
    final size = View.of(context).display.size;
    final pixels =
        widget.localResolution ?? (size.width.round(), size.height.round());
    return (
      (pixels.$1 / widget.outputPixelRatio).round(),
      (pixels.$2 / widget.outputPixelRatio).round()
    );
  }

  (int, int, int)? _localMode(BuildContext context) {
    if (!widget.scales.contains(_scale)) return null;
    final size = fitDisplayResolution(
        localSize: _localSize(context),
        minDimension: widget.minDimension,
        maxDimension: widget.maxDimension,
        scale: _scale,
        allowArbitrarySize: widget.allowArbitrarySize,
        supported: widget.supportedResolutions);
    return size == null ? null : (size.$1, size.$2, _scale);
  }

  void _setMode((int, int, int) mode) => setState(() {
        _scale = widget.scales.contains(mode.$3) ? mode.$3 : 1;
        _width.text = '${mode.$1 ~/ _scale}';
        _height.text = '${mode.$2 ~/ _scale}';
        _ratio = _ratioForSize(mode.$1 ~/ _scale, mode.$2 ~/ _scale);
      });

  Widget _shortcut({
    required String label,
    required IconData icon,
    required (int, int, int)? mode,
    String? hint,
  }) {
    final enabled = mode != null && _validSize(mode.$1, mode.$2, mode.$3);
    return Tooltip(
      message: [
        if (hint != null) widget.translate(hint),
        if (mode != null)
          _modeLabel(mode)
        else if (hint == null)
          widget.translate(label),
      ].join('\n'),
      child: TextButton.icon(
        onPressed: enabled ? () => _setMode(mode) : null,
        icon: Icon(icon, size: 18),
        label: Text(widget.translate(label)),
      ),
    );
  }

  Widget _quickSettings(BuildContext context) {
    final localMode = _localMode(context);
    final adjusted = localMode != null &&
        (localMode.$1, localMode.$2) != _localSize(context);
    final hint = localMode == null
        ? 'Local resolution is unavailable or unsupported'
        : adjusted
            ? widget.allowArbitrarySize
                ? 'Adjusted to display limits'
                : 'Closest supported resolution'
            : null;
    final defaultMode = widget.defaultResolution;
    return Wrap(spacing: 8, children: [
      _shortcut(
        label: 'resolution_fit_local_tip',
        icon: Icons.fit_screen,
        mode: localMode,
        hint: hint,
      ),
      if (defaultMode != null)
        _shortcut(
          label: widget.defaultLabel,
          icon: Icons.settings_backup_restore,
          mode: defaultMode,
        ),
    ]);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final sectionStyle = theme.textTheme.titleSmall;
    final busy = _applying || _systemScale?.busy == true;
    final scaleChanged = _systemScale?.changed == true;
    final canEditResolution =
        widget.allowArbitrarySize || widget.supportedResolutions.isNotEmpty;
    final showSystemScale = _systemScale != null &&
        (!_systemScale!.unavailable || !canEditResolution);
    final canApply = !busy &&
        (_valid || (!_hasChanges && scaleChanged)) &&
        (_systemScale?.canApply ?? true) &&
        (_hasChanges || scaleChanged);
    return preventMouseKeyBuilder(
      block: busy,
      child: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (showSystemScale) ...[
              DisplayScale(
                  excludeInputSemantics: widget.excludeInputSemantics,
                  translate: widget.translate,
                  controller: _systemScale!),
              if (canEditResolution) const Divider(height: 24),
            ],
            if (canEditResolution) ...[
              if (widget.usesLogicalSize)
                Text(widget.translate('Interface size'), style: sectionStyle),
              Wrap(
                alignment: WrapAlignment.spaceBetween,
                crossAxisAlignment: WrapCrossAlignment.center,
                spacing: 8,
                children: [
                  Text(
                      widget.translate(
                          widget.usesLogicalSize ? 'Looks like' : 'Resolution'),
                      style: widget.usesLogicalSize
                          ? theme.textTheme.bodyMedium
                          : sectionStyle),
                  _ratioSelector(),
                ],
              ),
              const SizedBox(height: 8),
              Row(children: [
                _dimension(_width, 'Width'),
                IconButton(
                  tooltip: widget.translate('Swap width and height'),
                  icon: const Icon(Icons.swap_horiz),
                  onPressed: !_validSize(_editedMode.$2, _editedMode.$1, _scale)
                      ? null
                      : () => setState(() {
                            final width = _width.text;
                            _width.text = _height.text;
                            _height.text = width;
                            final ratio = _ratio;
                            if (ratio != null) _ratio = (ratio.$2, ratio.$1);
                          }),
                ),
                _dimension(_height, 'Height'),
              ]),
              if (!_valid && (_hasChanges || _systemScale == null))
                Text(
                  '${widget.translate(widget.allowArbitrarySize ? 'Enter dimensions within the range' : 'Select a resolution supported by the display')}'
                  '${widget.allowArbitrarySize ? ' ($_minDimension–$_maxDimension px)' : ''}',
                  style: TextStyle(color: theme.colorScheme.error),
                ),
              if (_hasChanges) ...[
                const SizedBox(height: 8),
                Text(
                    '${widget.translate(widget.usesLogicalSize ? 'Current interface size' : 'Current resolution')}: '
                    '${_modeLabel(_currentMode)}',
                    style: theme.textTheme.bodySmall),
              ],
              const SizedBox(height: 16),
              _quickSettings(context),
              if (widget.scales.length > 1) ...[
                const Divider(height: 24),
                Wrap(
                  spacing: 8,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Text(widget.translate('Rendering quality'),
                        style: sectionStyle),
                    Wrap(spacing: 8, children: [
                      for (final scale in widget.scales)
                        ChoiceChip(
                          label: Text(_qualityLabel(scale)),
                          selected: _scale == scale,
                          onSelected: (_) => setState(() => _scale = scale),
                        ),
                    ]),
                  ],
                ),
                if (_valid)
                  Text(
                      '${widget.translate('Output resolution')}: '
                      '${_dimensionLabel(_editedMode.$1, _editedMode.$2)} px',
                      style: theme.textTheme.bodySmall),
              ],
            ],
            if (_applying && _systemScale?.busy != true)
              const LinearProgressIndicator(),
            if (!busy && _resolutionApplied)
              Text(
                  '${widget.translate('Resolution')}: ${widget.translate('Successful')} '
                  '(${_modeLabel(_currentMode)})',
                  style: theme.textTheme.bodySmall),
            if (_applyError != null) ...[
              const SizedBox(height: 12),
              Text(widget.translate(_applyError!),
                  style: TextStyle(color: theme.colorScheme.error)),
            ],
            const Divider(height: 24),
            Wrap(
                alignment: WrapAlignment.spaceBetween,
                spacing: 8,
                runSpacing: 4,
                children: [
                  TextButton(
                    onPressed: !busy &&
                            (_hasEdits ||
                                scaleChanged ||
                                _systemScale?.customMode == true ||
                                _systemScale?.valid == false)
                        ? () {
                            _setMode(_currentMode);
                            _systemScale?.reset();
                            setState(() => _applyError = null);
                          }
                        : null,
                    child: Text(widget.translate('Reset changes')),
                  ),
                  Wrap(spacing: 8, children: [
                    TextButton(
                      onPressed: busy ? null : widget.onCancel,
                      child: Text(widget.translate(
                          _resolutionApplied ? 'Close' : 'Cancel')),
                    ),
                    ElevatedButton(
                      onPressed: canApply ? _apply : null,
                      child: Text(widget.translate('Apply')),
                    ),
                  ]),
                ]),
          ],
        ),
      ),
    );
  }
}
