import 'package:flutter/gestures.dart';

class MultiTouchGestureRecognizer extends MultiTapGestureRecognizer {
  MultiTouchGestureRecognizerCallback onMultiTap;
  var numberOfTouches = 0;

  MultiTouchGestureRecognizer() {
    this
      ..onTapDown = addTouch
      ..onTapUp = removeTouch
      ..onTapCancel = cancelTouch
      ..onTap = captureDefaultTap;
  }

  void addTouch(int pointer, TapDownDetails details) {
    numberOfTouches++;
    onMultiTap(numberOfTouches, true);
  }

  void removeTouch(int pointer, TapUpDetails details) {
    numberOfTouches--;
    onMultiTap(numberOfTouches, false);
  }

  void cancelTouch(int pointer) {
    numberOfTouches = 0;
  }

  void captureDefaultTap(int pointer) {}
}

typedef MultiTouchGestureRecognizerCallback = void Function(
    int touchCount, bool addOrRemove);

typedef OnUpdate(DragUpdateDetails details);

class CustomMultiDrag extends Drag {
  CustomMultiDrag({this.events, this.offset});

  List<PointerDownEvent> events;
  Offset offset;

  @override
  void update(DragUpdateDetails details) {
    var n = events.length;
    print('$n $details');
  }

  @override
  void end(DragEndDetails details) {
    super.end(details);
  }
}

typedef OnDisposeState();

// clone _ImmediatePointerState
class CustomPointerState extends MultiDragPointerState {
  final OnDisposeState onDisposeState;
  CustomPointerState(Offset initialPosition, PointerDeviceKind kind,
      {this.onDisposeState})
      : super(initialPosition, kind);

  @override
  void checkForResolutionAfterMove() {
    assert(pendingDelta != null);
    if (pendingDelta.distance > computeHitSlop(kind))
      resolve(GestureDisposition.accepted);
  }

  @override
  void accepted(GestureMultiDragStartCallback starter) {
    starter(initialPosition);
  }

  @override
  void dispose() {
    onDisposeState.call();
    super.dispose();
  }
}

// clone ImmediateMultiDragGestureRecognizer
class CustomMultiDragGestureRecognizer
    extends MultiDragGestureRecognizer<CustomPointerState> {
  var events = List<PointerDownEvent>();

  /// Create a gesture recognizer for tracking multiple pointers at once.
  CustomMultiDragGestureRecognizer({
    Object debugOwner,
    PointerDeviceKind kind,
  }) : super(debugOwner: debugOwner, kind: kind);

  @override
  CustomPointerState createNewPointerState(PointerDownEvent event) {
    events.add(event);
    return CustomPointerState(event.position, event.kind, onDisposeState: () {
      events.remove(event);
    });
  }

  @override
  String get debugDescription => 'custom_multidrag';
}
