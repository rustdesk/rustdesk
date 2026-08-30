import 'package:xterm/src/base/disposable.dart';

typedef EventListener<T> = void Function(T event);

class Event<T> {
  final EventEmitter<T> emitter;

  Event(this.emitter);

  void call(EventListener<T> listener) {
    emitter(listener);
  }
}

class EventEmitter<T> {
  final _listeners = <EventListener<T>>[];

  EventSubscription<T> call(EventListener<T> listener) {
    _listeners.add(listener);
    return EventSubscription(this, listener);
  }

  void emit(T event) {
    for (final listener in _listeners) {
      listener(event);
    }
  }

  Event<T> get event => Event(this);
}

class EventSubscription<T> with Disposable {
  final EventEmitter<T> emitter;
  final EventListener<T> listener;

  EventSubscription(this.emitter, this.listener);

  @override
  void dispose() {
    emitter._listeners.remove(listener);
  }
}
