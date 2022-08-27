// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target

part of 'generated_bridge.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
    'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#custom-getters-and-methods');

/// @nodoc
mixin _$EventToUI {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String field0) event,
    required TResult Function(Uint8List field0) rgba,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult Function(String field0)? event,
    TResult Function(Uint8List field0)? rgba,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String field0)? event,
    TResult Function(Uint8List field0)? rgba,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event value) event,
    required TResult Function(Rgba value) rgba,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult Function(Event value)? event,
    TResult Function(Rgba value)? rgba,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event value)? event,
    TResult Function(Rgba value)? rgba,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $EventToUICopyWith<$Res> {
  factory $EventToUICopyWith(EventToUI value, $Res Function(EventToUI) then) =
      _$EventToUICopyWithImpl<$Res>;
}

/// @nodoc
class _$EventToUICopyWithImpl<$Res> implements $EventToUICopyWith<$Res> {
  _$EventToUICopyWithImpl(this._value, this._then);

  final EventToUI _value;
  // ignore: unused_field
  final $Res Function(EventToUI) _then;
}

/// @nodoc
abstract class _$$EventCopyWith<$Res> {
  factory _$$EventCopyWith(_$Event value, $Res Function(_$Event) then) =
      __$$EventCopyWithImpl<$Res>;
  $Res call({String field0});
}

/// @nodoc
class __$$EventCopyWithImpl<$Res> extends _$EventToUICopyWithImpl<$Res>
    implements _$$EventCopyWith<$Res> {
  __$$EventCopyWithImpl(_$Event _value, $Res Function(_$Event) _then)
      : super(_value, (v) => _then(v as _$Event));

  @override
  _$Event get _value => super._value as _$Event;

  @override
  $Res call({
    Object? field0 = freezed,
  }) {
    return _then(_$Event(
      field0 == freezed
          ? _value.field0
          : field0 // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// @nodoc

class _$Event implements Event {
  const _$Event(this.field0);

  @override
  final String field0;

  @override
  String toString() {
    return 'EventToUI.event(field0: $field0)';
  }

  @override
  bool operator ==(dynamic other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Event &&
            const DeepCollectionEquality().equals(other.field0, field0));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(field0));

  @JsonKey(ignore: true)
  @override
  _$$EventCopyWith<_$Event> get copyWith =>
      __$$EventCopyWithImpl<_$Event>(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String field0) event,
    required TResult Function(Uint8List field0) rgba,
  }) {
    return event(field0);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult Function(String field0)? event,
    TResult Function(Uint8List field0)? rgba,
  }) {
    return event?.call(field0);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String field0)? event,
    TResult Function(Uint8List field0)? rgba,
    required TResult orElse(),
  }) {
    if (event != null) {
      return event(field0);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event value) event,
    required TResult Function(Rgba value) rgba,
  }) {
    return event(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult Function(Event value)? event,
    TResult Function(Rgba value)? rgba,
  }) {
    return event?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event value)? event,
    TResult Function(Rgba value)? rgba,
    required TResult orElse(),
  }) {
    if (event != null) {
      return event(this);
    }
    return orElse();
  }
}

abstract class Event implements EventToUI {
  const factory Event(final String field0) = _$Event;

  String get field0;
  @JsonKey(ignore: true)
  _$$EventCopyWith<_$Event> get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$RgbaCopyWith<$Res> {
  factory _$$RgbaCopyWith(_$Rgba value, $Res Function(_$Rgba) then) =
      __$$RgbaCopyWithImpl<$Res>;
  $Res call({Uint8List field0});
}

/// @nodoc
class __$$RgbaCopyWithImpl<$Res> extends _$EventToUICopyWithImpl<$Res>
    implements _$$RgbaCopyWith<$Res> {
  __$$RgbaCopyWithImpl(_$Rgba _value, $Res Function(_$Rgba) _then)
      : super(_value, (v) => _then(v as _$Rgba));

  @override
  _$Rgba get _value => super._value as _$Rgba;

  @override
  $Res call({
    Object? field0 = freezed,
  }) {
    return _then(_$Rgba(
      field0 == freezed
          ? _value.field0
          : field0 // ignore: cast_nullable_to_non_nullable
              as Uint8List,
    ));
  }
}

/// @nodoc

class _$Rgba implements Rgba {
  const _$Rgba(this.field0);

  @override
  final Uint8List field0;

  @override
  String toString() {
    return 'EventToUI.rgba(field0: $field0)';
  }

  @override
  bool operator ==(dynamic other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$Rgba &&
            const DeepCollectionEquality().equals(other.field0, field0));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(field0));

  @JsonKey(ignore: true)
  @override
  _$$RgbaCopyWith<_$Rgba> get copyWith =>
      __$$RgbaCopyWithImpl<_$Rgba>(this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String field0) event,
    required TResult Function(Uint8List field0) rgba,
  }) {
    return rgba(field0);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult Function(String field0)? event,
    TResult Function(Uint8List field0)? rgba,
  }) {
    return rgba?.call(field0);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String field0)? event,
    TResult Function(Uint8List field0)? rgba,
    required TResult orElse(),
  }) {
    if (rgba != null) {
      return rgba(field0);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(Event value) event,
    required TResult Function(Rgba value) rgba,
  }) {
    return rgba(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult Function(Event value)? event,
    TResult Function(Rgba value)? rgba,
  }) {
    return rgba?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(Event value)? event,
    TResult Function(Rgba value)? rgba,
    required TResult orElse(),
  }) {
    if (rgba != null) {
      return rgba(this);
    }
    return orElse();
  }
}

abstract class Rgba implements EventToUI {
  const factory Rgba(final Uint8List field0) = _$Rgba;

  Uint8List get field0;
  @JsonKey(ignore: true)
  _$$RgbaCopyWith<_$Rgba> get copyWith => throw _privateConstructorUsedError;
}
