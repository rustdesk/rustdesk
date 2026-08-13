import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';

import '../../common.dart';
import '../../consts.dart';
import '../../models/platform_model.dart';
import '../../models/state_model.dart';

import 'package:texture_rgba_renderer/texture_rgba_renderer.dart'
    if (dart.library.html) 'package:flutter_hbb/web/texture_rgba_renderer.dart';

/// Startup probe: renders one frame through a 1x1 external texture and
/// records in `texture-render-health` whether the engine consumed it, so a
/// broken environment is detected before the first session goes black.
class TextureRenderProbe extends StatefulWidget {
  const TextureRenderProbe({Key? key}) : super(key: key);

  @override
  State<TextureRenderProbe> createState() => _TextureRenderProbeState();
}

class _TextureRenderProbeState extends State<TextureRenderProbe> {
  static bool _ranThisLaunch = false;
  final _renderer = TextureRgbaRenderer();
  int _textureId = -1;
  int _textureKey = -1;
  int _ptr = 0;
  Timer? _timer;
  int _ticks = 0;
  bool _sawTimings = false;
  bool _wasEffectiveOn = false;
  DateTime? _lastTimings;

  @override
  void initState() {
    super.initState();
    if (_ranThisLaunch || isWeb || !isDesktop) return;
    _ranThisLaunch = true;
    if (bind.isIncomingOnly()) return;
    // An old plugin without the consumed counter cannot be judged, and a
    // recorded raster-stall means compositing a texture may hang this window.
    if (!bind.mainTextureRenderProbeSupported()) return;
    if (bind
        .mainGetLocalOption(key: kOptionTextureRenderHealth)
        .startsWith('failed-raster-stall')) {
      return;
    }
    _wasEffectiveOn = bind.mainGetUseTextureRender();
    // Only probe after the window has really rendered a frame: a hidden
    // window (silent/tray start) must not record a false failure.
    SchedulerBinding.instance.addTimingsCallback(_onTimings);
    Future.delayed(const Duration(seconds: 5), () {
      if (!_sawTimings) {
        SchedulerBinding.instance.removeTimingsCallback(_onTimings);
        _finish(null);
      }
    });
  }

  void _onTimings(List<FrameTiming> timings) {
    _lastTimings = DateTime.now();
    if (_sawTimings) return;
    _sawTimings = true;
    _start();
  }

  void _start() async {
    if (!mounted) return;
    _textureKey = bind.getNextTextureKey();
    final id = await _renderer.createTexture(_textureKey);
    if (!mounted || id == -1) {
      _finish(!mounted ? null : false);
      return;
    }
    _ptr = await _renderer.getTexturePtr(_textureKey);
    if (!mounted || _ptr <= 0) {
      _finish(!mounted ? null : false);
      return;
    }
    setState(() => _textureId = id);
    _timer = Timer.periodic(const Duration(milliseconds: 100), (_) {
      _ticks += 1;
      bind.mainPushTextureProbeFrame(ptr: _ptr);
      if (bind.mainGetTextureProbeConsumed(ptr: _ptr) > 0) {
        _finish(true);
      } else if (_ticks >= 10) {
        // Only a window that is visibly compositing can prove a failure.
        final timingsFresh = _lastTimings != null &&
            DateTime.now().difference(_lastTimings!) <
                const Duration(milliseconds: 1500);
        _finish(!stateGlobal.isMinimized && timingsFresh ? false : null);
      }
    });
  }

  void _finish(bool? ok) {
    _timer?.cancel();
    _timer = null;
    SchedulerBinding.instance.removeTimingsCallback(_onTimings);
    if (ok != null) {
      final old = bind.mainGetLocalOption(key: kOptionTextureRenderHealth);
      if (ok) {
        // A 1x1 probe pass disproves the black-texture class, not a
        // raster-stall under load; that record only clears via the toggle.
        if (old != 'ok' && !old.startsWith('failed-raster-stall')) {
          bind.mainSetLocalOption(key: kOptionTextureRenderHealth, value: 'ok');
        }
      } else if (!old.startsWith('failed')) {
        debugPrint('texture render probe failed, disabling texture rendering');
        bind.mainSetLocalOption(
            key: kOptionTextureRenderHealth, value: 'failed-probe');
        if (_wasEffectiveOn) {
          showToast(translate('texture-render-fallback-tip'));
        }
      }
    }
    if (_textureKey != -1) {
      _renderer.closeTexture(_textureKey);
      _textureKey = -1;
    }
    _ptr = 0;
    if (mounted && _textureId != -1) {
      setState(() => _textureId = -1);
    } else {
      _textureId = -1;
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    SchedulerBinding.instance.removeTimingsCallback(_onTimings);
    if (_textureKey != -1) {
      _renderer.closeTexture(_textureKey);
      _textureKey = -1;
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_textureId == -1) return const SizedBox.shrink();
    // Must actually composite for the engine to sample the texture; the
    // pushed pixel is fully transparent.
    return IgnorePointer(
      child: SizedBox(
          width: 1, height: 1, child: Texture(textureId: _textureId)),
    );
  }
}
