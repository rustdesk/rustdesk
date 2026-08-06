import 'dart:async';

typedef VideoFrameImporter<Frame, Image> = Future<Image> Function(Frame frame);
typedef VideoFrameCloser<Frame> = void Function(Frame frame);
typedef VideoImageDisposer<Image> = void Function(Image image);
typedef VideoSessionValidator = bool Function();
typedef VideoImageCallback<Image> = Future<void> Function(
    int display, Image image, VideoSessionValidator isCurrentSession);
typedef VideoQueueErrorCallback = void Function(
    Object error, StackTrace stackTrace);

class WebVideoFrameQueue<Frame, Image> {
  WebVideoFrameQueue({
    required VideoFrameImporter<Frame, Image> importFrame,
    required VideoFrameCloser<Frame> closeFrame,
    required VideoImageDisposer<Image> disposeImage,
    required VideoQueueErrorCallback onImportError,
    required VideoQueueErrorCallback onCallbackError,
  })  : _importFrame = importFrame,
        _closeFrame = closeFrame,
        _disposeImage = disposeImage,
        _onImportError = onImportError,
        _onCallbackError = onCallbackError;

  final VideoFrameImporter<Frame, Image> _importFrame;
  final VideoFrameCloser<Frame> _closeFrame;
  final VideoImageDisposer<Image> _disposeImage;
  final VideoQueueErrorCallback _onImportError;
  final VideoQueueErrorCallback _onCallbackError;
  final Map<int, _QueuedFrame<Frame>> _pending = {};

  VideoImageCallback<Image>? _callback;
  int _generation = 0;
  bool _processing = false;
  bool _enabled = true;

  bool get isEnabled => _enabled;

  void beginSession(VideoImageCallback<Image> callback) {
    _invalidateSession();
    _enabled = true;
    _callback = callback;
  }

  void endSession() {
    _invalidateSession();
    _callback = null;
  }

  void _invalidateSession() {
    _generation++;
    for (final queued in _pending.values) {
      _closeFrame(queued.frame);
    }
    _pending.clear();
  }

  bool submit(int display, Frame frame) {
    if (!_enabled || _callback == null) {
      _closeFrame(frame);
      return false;
    }
    final replaced = _pending.remove(display);
    if (replaced != null) {
      _closeFrame(replaced.frame);
    }
    _pending[display] = _QueuedFrame(display, frame, _generation);
    _startProcessing();
    return true;
  }

  void _startProcessing() {
    if (_processing) return;
    _processing = true;
    unawaited(Future<void>(_process));
  }

  Future<void> _process() async {
    while (_pending.isNotEmpty) {
      final display = _pending.keys.first;
      final queued = _pending.remove(display)!;
      if (!_enabled || queued.generation != _generation) {
        _closeFrame(queued.frame);
        continue;
      }
      await _importAndDeliver(queued);
    }
    _processing = false;
  }

  Future<void> _importAndDeliver(_QueuedFrame<Frame> queued) async {
    Image? image;
    try {
      image = await _importFrame(queued.frame);
    } catch (error, stackTrace) {
      if (queued.generation == _generation) {
        _enabled = false;
        _onImportError(error, stackTrace);
      }
    } finally {
      _closeFrame(queued.frame);
    }
    if (image != null) {
      await _deliver(queued, image);
    }
  }

  Future<void> _deliver(_QueuedFrame<Frame> queued, Image image) async {
    final callback = _callback;
    bool isCurrentSession() =>
        _enabled &&
        queued.generation == _generation &&
        identical(callback, _callback);
    if (!isCurrentSession() || callback == null) {
      _disposeImage(image);
      return;
    }
    try {
      await callback(queued.display, image, isCurrentSession);
    } catch (error, stackTrace) {
      _disposeImage(image);
      _onCallbackError(error, stackTrace);
    }
  }
}

class _QueuedFrame<Frame> {
  const _QueuedFrame(this.display, this.frame, this.generation);

  final int display;
  final Frame frame;
  final int generation;
}
