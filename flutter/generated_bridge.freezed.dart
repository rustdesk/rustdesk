// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'generated_bridge.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
    'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#custom-getters-and-methods');

/// @nodoc
mixin _$EventToUI {
  Object get field0 => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String field0) event,
    required TResult Function(int field0) rgba,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String field0)? event,
    TResult? Function(int field0)? rgba,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String field0)? event,
    TResult Function(int field0)? rgba,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(EventToUI_Event value) event,
    required TResult Function(EventToUI_Rgba value) rgba,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(EventToUI_Event value)? event,
    TResult? Function(EventToUI_Rgba value)? rgba,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(EventToUI_Event value)? event,
    TResult Function(EventToUI_Rgba value)? rgba,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $EventToUICopyWith<$Res> {
  factory $EventToUICopyWith(EventToUI value, $Res Function(EventToUI) then) =
      _$EventToUICopyWithImpl<$Res, EventToUI>;
}

/// @nodoc
class _$EventToUICopyWithImpl<$Res, $Val extends EventToUI>
    implements $EventToUICopyWith<$Res> {
  _$EventToUICopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;
}

/// @nodoc
abstract class _$$EventToUI_EventImplCopyWith<$Res> {
  factory _$$EventToUI_EventImplCopyWith(_$EventToUI_EventImpl value,
          $Res Function(_$EventToUI_EventImpl) then) =
      __$$EventToUI_EventImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String field0});
}

/// @nodoc
class __$$EventToUI_EventImplCopyWithImpl<$Res>
    extends _$EventToUICopyWithImpl<$Res, _$EventToUI_EventImpl>
    implements _$$EventToUI_EventImplCopyWith<$Res> {
  __$$EventToUI_EventImplCopyWithImpl(
      _$EventToUI_EventImpl _value, $Res Function(_$EventToUI_EventImpl) _then)
      : super(_value, _then);

  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? field0 = null,
  }) {
    return _then(_$EventToUI_EventImpl(
      null == field0
          ? _value.field0
          : field0 // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// @nodoc

class _$EventToUI_EventImpl implements EventToUI_Event {
  const _$EventToUI_EventImpl(this.field0);

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
            other is _$EventToUI_EventImpl &&
            (identical(other.field0, field0) || other.field0 == field0));
  }

  @override
  int get hashCode => Object.hash(runtimeType, field0);

  @JsonKey(ignore: true)
  @override
  @pragma('vm:prefer-inline')
  _$$EventToUI_EventImplCopyWith<_$EventToUI_EventImpl> get copyWith =>
      __$$EventToUI_EventImplCopyWithImpl<_$EventToUI_EventImpl>(
          this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String field0) event,
    required TResult Function(int field0) rgba,
  }) {
    return event(field0);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String field0)? event,
    TResult? Function(int field0)? rgba,
  }) {
    return event?.call(field0);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String field0)? event,
    TResult Function(int field0)? rgba,
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
    required TResult Function(EventToUI_Event value) event,
    required TResult Function(EventToUI_Rgba value) rgba,
  }) {
    return event(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(EventToUI_Event value)? event,
    TResult? Function(EventToUI_Rgba value)? rgba,
  }) {
    return event?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(EventToUI_Event value)? event,
    TResult Function(EventToUI_Rgba value)? rgba,
    required TResult orElse(),
  }) {
    if (event != null) {
      return event(this);
    }
    return orElse();
  }
}

abstract class EventToUI_Event implements EventToUI {
  const factory EventToUI_Event(final String field0) = _$EventToUI_EventImpl;

  @override
  String get field0;
  @JsonKey(ignore: true)
  _$$EventToUI_EventImplCopyWith<_$EventToUI_EventImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$EventToUI_RgbaImplCopyWith<$Res> {
  factory _$$EventToUI_RgbaImplCopyWith(_$EventToUI_RgbaImpl value,
          $Res Function(_$EventToUI_RgbaImpl) then) =
      __$$EventToUI_RgbaImplCopyWithImpl<$Res>;
  @useResult
  $Res call({int field0});
}

/// @nodoc
class __$$EventToUI_RgbaImplCopyWithImpl<$Res>
    extends _$EventToUICopyWithImpl<$Res, _$EventToUI_RgbaImpl>
    implements _$$EventToUI_RgbaImplCopyWith<$Res> {
  __$$EventToUI_RgbaImplCopyWithImpl(
      _$EventToUI_RgbaImpl _value, $Res Function(_$EventToUI_RgbaImpl) _then)
      : super(_value, _then);

  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? field0 = null,
  }) {
    return _then(_$EventToUI_RgbaImpl(
      null == field0
          ? _value.field0
          : field0 // ignore: cast_nullable_to_non_nullable
              as int,
    ));
  }
}

/// @nodoc

class _$EventToUI_RgbaImpl implements EventToUI_Rgba {
  const _$EventToUI_RgbaImpl(this.field0);

  @override
  final int field0;

  @override
  String toString() {
    return 'EventToUI.rgba(field0: $field0)';
  }

  @override
  bool operator ==(dynamic other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$EventToUI_RgbaImpl &&
            (identical(other.field0, field0) || other.field0 == field0));
  }

  @override
  int get hashCode => Object.hash(runtimeType, field0);

  @JsonKey(ignore: true)
  @override
  @pragma('vm:prefer-inline')
  _$$EventToUI_RgbaImplCopyWith<_$EventToUI_RgbaImpl> get copyWith =>
      __$$EventToUI_RgbaImplCopyWithImpl<_$EventToUI_RgbaImpl>(
          this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String field0) event,
    required TResult Function(int field0) rgba,
  }) {
    return rgba(field0);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String field0)? event,
    TResult? Function(int field0)? rgba,
  }) {
    return rgba?.call(field0);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String field0)? event,
    TResult Function(int field0)? rgba,
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
    required TResult Function(EventToUI_Event value) event,
    required TResult Function(EventToUI_Rgba value) rgba,
  }) {
    return rgba(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(EventToUI_Event value)? event,
    TResult? Function(EventToUI_Rgba value)? rgba,
  }) {
    return rgba?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(EventToUI_Event value)? event,
    TResult Function(EventToUI_Rgba value)? rgba,
    required TResult orElse(),
  }) {
    if (rgba != null) {
      return rgba(this);
    }
    return orElse();
  }
}

abstract class EventToUI_Rgba implements EventToUI {
  const factory EventToUI_Rgba(final int field0) = _$EventToUI_RgbaImpl;

  @override
  int get field0;
  @JsonKey(ignore: true)
  _$$EventToUI_RgbaImplCopyWith<_$EventToUI_RgbaImpl> get copyWith =>
      throw _privateConstructorUsedError;
}
