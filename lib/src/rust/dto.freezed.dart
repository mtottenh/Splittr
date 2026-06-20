// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'dto.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
  'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#adding-getters-and-methods-to-our-models',
);

/// @nodoc
mixin _$SplitPlanDto {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> participants) equal,
    required TResult Function(List<Payer> amounts) exact,
    required TResult Function(List<Weight> weights) weighted,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> participants)? equal,
    TResult? Function(List<Payer> amounts)? exact,
    TResult? Function(List<Weight> weights)? weighted,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> participants)? equal,
    TResult Function(List<Payer> amounts)? exact,
    TResult Function(List<Weight> weights)? weighted,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(SplitPlanDto_Equal value) equal,
    required TResult Function(SplitPlanDto_Exact value) exact,
    required TResult Function(SplitPlanDto_Weighted value) weighted,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(SplitPlanDto_Equal value)? equal,
    TResult? Function(SplitPlanDto_Exact value)? exact,
    TResult? Function(SplitPlanDto_Weighted value)? weighted,
  }) => throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(SplitPlanDto_Equal value)? equal,
    TResult Function(SplitPlanDto_Exact value)? exact,
    TResult Function(SplitPlanDto_Weighted value)? weighted,
    required TResult orElse(),
  }) => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $SplitPlanDtoCopyWith<$Res> {
  factory $SplitPlanDtoCopyWith(
    SplitPlanDto value,
    $Res Function(SplitPlanDto) then,
  ) = _$SplitPlanDtoCopyWithImpl<$Res, SplitPlanDto>;
}

/// @nodoc
class _$SplitPlanDtoCopyWithImpl<$Res, $Val extends SplitPlanDto>
    implements $SplitPlanDtoCopyWith<$Res> {
  _$SplitPlanDtoCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of SplitPlanDto
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc
abstract class _$$SplitPlanDto_EqualImplCopyWith<$Res> {
  factory _$$SplitPlanDto_EqualImplCopyWith(
    _$SplitPlanDto_EqualImpl value,
    $Res Function(_$SplitPlanDto_EqualImpl) then,
  ) = __$$SplitPlanDto_EqualImplCopyWithImpl<$Res>;
  @useResult
  $Res call({List<String> participants});
}

/// @nodoc
class __$$SplitPlanDto_EqualImplCopyWithImpl<$Res>
    extends _$SplitPlanDtoCopyWithImpl<$Res, _$SplitPlanDto_EqualImpl>
    implements _$$SplitPlanDto_EqualImplCopyWith<$Res> {
  __$$SplitPlanDto_EqualImplCopyWithImpl(
    _$SplitPlanDto_EqualImpl _value,
    $Res Function(_$SplitPlanDto_EqualImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of SplitPlanDto
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? participants = null}) {
    return _then(
      _$SplitPlanDto_EqualImpl(
        participants: null == participants
            ? _value._participants
            : participants // ignore: cast_nullable_to_non_nullable
                  as List<String>,
      ),
    );
  }
}

/// @nodoc

class _$SplitPlanDto_EqualImpl extends SplitPlanDto_Equal {
  const _$SplitPlanDto_EqualImpl({required final List<String> participants})
    : _participants = participants,
      super._();

  final List<String> _participants;
  @override
  List<String> get participants {
    if (_participants is EqualUnmodifiableListView) return _participants;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_participants);
  }

  @override
  String toString() {
    return 'SplitPlanDto.equal(participants: $participants)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$SplitPlanDto_EqualImpl &&
            const DeepCollectionEquality().equals(
              other._participants,
              _participants,
            ));
  }

  @override
  int get hashCode => Object.hash(
    runtimeType,
    const DeepCollectionEquality().hash(_participants),
  );

  /// Create a copy of SplitPlanDto
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$SplitPlanDto_EqualImplCopyWith<_$SplitPlanDto_EqualImpl> get copyWith =>
      __$$SplitPlanDto_EqualImplCopyWithImpl<_$SplitPlanDto_EqualImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> participants) equal,
    required TResult Function(List<Payer> amounts) exact,
    required TResult Function(List<Weight> weights) weighted,
  }) {
    return equal(participants);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> participants)? equal,
    TResult? Function(List<Payer> amounts)? exact,
    TResult? Function(List<Weight> weights)? weighted,
  }) {
    return equal?.call(participants);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> participants)? equal,
    TResult Function(List<Payer> amounts)? exact,
    TResult Function(List<Weight> weights)? weighted,
    required TResult orElse(),
  }) {
    if (equal != null) {
      return equal(participants);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(SplitPlanDto_Equal value) equal,
    required TResult Function(SplitPlanDto_Exact value) exact,
    required TResult Function(SplitPlanDto_Weighted value) weighted,
  }) {
    return equal(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(SplitPlanDto_Equal value)? equal,
    TResult? Function(SplitPlanDto_Exact value)? exact,
    TResult? Function(SplitPlanDto_Weighted value)? weighted,
  }) {
    return equal?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(SplitPlanDto_Equal value)? equal,
    TResult Function(SplitPlanDto_Exact value)? exact,
    TResult Function(SplitPlanDto_Weighted value)? weighted,
    required TResult orElse(),
  }) {
    if (equal != null) {
      return equal(this);
    }
    return orElse();
  }
}

abstract class SplitPlanDto_Equal extends SplitPlanDto {
  const factory SplitPlanDto_Equal({required final List<String> participants}) =
      _$SplitPlanDto_EqualImpl;
  const SplitPlanDto_Equal._() : super._();

  List<String> get participants;

  /// Create a copy of SplitPlanDto
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$SplitPlanDto_EqualImplCopyWith<_$SplitPlanDto_EqualImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$SplitPlanDto_ExactImplCopyWith<$Res> {
  factory _$$SplitPlanDto_ExactImplCopyWith(
    _$SplitPlanDto_ExactImpl value,
    $Res Function(_$SplitPlanDto_ExactImpl) then,
  ) = __$$SplitPlanDto_ExactImplCopyWithImpl<$Res>;
  @useResult
  $Res call({List<Payer> amounts});
}

/// @nodoc
class __$$SplitPlanDto_ExactImplCopyWithImpl<$Res>
    extends _$SplitPlanDtoCopyWithImpl<$Res, _$SplitPlanDto_ExactImpl>
    implements _$$SplitPlanDto_ExactImplCopyWith<$Res> {
  __$$SplitPlanDto_ExactImplCopyWithImpl(
    _$SplitPlanDto_ExactImpl _value,
    $Res Function(_$SplitPlanDto_ExactImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of SplitPlanDto
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? amounts = null}) {
    return _then(
      _$SplitPlanDto_ExactImpl(
        amounts: null == amounts
            ? _value._amounts
            : amounts // ignore: cast_nullable_to_non_nullable
                  as List<Payer>,
      ),
    );
  }
}

/// @nodoc

class _$SplitPlanDto_ExactImpl extends SplitPlanDto_Exact {
  const _$SplitPlanDto_ExactImpl({required final List<Payer> amounts})
    : _amounts = amounts,
      super._();

  final List<Payer> _amounts;
  @override
  List<Payer> get amounts {
    if (_amounts is EqualUnmodifiableListView) return _amounts;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_amounts);
  }

  @override
  String toString() {
    return 'SplitPlanDto.exact(amounts: $amounts)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$SplitPlanDto_ExactImpl &&
            const DeepCollectionEquality().equals(other._amounts, _amounts));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(_amounts));

  /// Create a copy of SplitPlanDto
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$SplitPlanDto_ExactImplCopyWith<_$SplitPlanDto_ExactImpl> get copyWith =>
      __$$SplitPlanDto_ExactImplCopyWithImpl<_$SplitPlanDto_ExactImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> participants) equal,
    required TResult Function(List<Payer> amounts) exact,
    required TResult Function(List<Weight> weights) weighted,
  }) {
    return exact(amounts);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> participants)? equal,
    TResult? Function(List<Payer> amounts)? exact,
    TResult? Function(List<Weight> weights)? weighted,
  }) {
    return exact?.call(amounts);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> participants)? equal,
    TResult Function(List<Payer> amounts)? exact,
    TResult Function(List<Weight> weights)? weighted,
    required TResult orElse(),
  }) {
    if (exact != null) {
      return exact(amounts);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(SplitPlanDto_Equal value) equal,
    required TResult Function(SplitPlanDto_Exact value) exact,
    required TResult Function(SplitPlanDto_Weighted value) weighted,
  }) {
    return exact(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(SplitPlanDto_Equal value)? equal,
    TResult? Function(SplitPlanDto_Exact value)? exact,
    TResult? Function(SplitPlanDto_Weighted value)? weighted,
  }) {
    return exact?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(SplitPlanDto_Equal value)? equal,
    TResult Function(SplitPlanDto_Exact value)? exact,
    TResult Function(SplitPlanDto_Weighted value)? weighted,
    required TResult orElse(),
  }) {
    if (exact != null) {
      return exact(this);
    }
    return orElse();
  }
}

abstract class SplitPlanDto_Exact extends SplitPlanDto {
  const factory SplitPlanDto_Exact({required final List<Payer> amounts}) =
      _$SplitPlanDto_ExactImpl;
  const SplitPlanDto_Exact._() : super._();

  List<Payer> get amounts;

  /// Create a copy of SplitPlanDto
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$SplitPlanDto_ExactImplCopyWith<_$SplitPlanDto_ExactImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$SplitPlanDto_WeightedImplCopyWith<$Res> {
  factory _$$SplitPlanDto_WeightedImplCopyWith(
    _$SplitPlanDto_WeightedImpl value,
    $Res Function(_$SplitPlanDto_WeightedImpl) then,
  ) = __$$SplitPlanDto_WeightedImplCopyWithImpl<$Res>;
  @useResult
  $Res call({List<Weight> weights});
}

/// @nodoc
class __$$SplitPlanDto_WeightedImplCopyWithImpl<$Res>
    extends _$SplitPlanDtoCopyWithImpl<$Res, _$SplitPlanDto_WeightedImpl>
    implements _$$SplitPlanDto_WeightedImplCopyWith<$Res> {
  __$$SplitPlanDto_WeightedImplCopyWithImpl(
    _$SplitPlanDto_WeightedImpl _value,
    $Res Function(_$SplitPlanDto_WeightedImpl) _then,
  ) : super(_value, _then);

  /// Create a copy of SplitPlanDto
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({Object? weights = null}) {
    return _then(
      _$SplitPlanDto_WeightedImpl(
        weights: null == weights
            ? _value._weights
            : weights // ignore: cast_nullable_to_non_nullable
                  as List<Weight>,
      ),
    );
  }
}

/// @nodoc

class _$SplitPlanDto_WeightedImpl extends SplitPlanDto_Weighted {
  const _$SplitPlanDto_WeightedImpl({required final List<Weight> weights})
    : _weights = weights,
      super._();

  final List<Weight> _weights;
  @override
  List<Weight> get weights {
    if (_weights is EqualUnmodifiableListView) return _weights;
    // ignore: implicit_dynamic_type
    return EqualUnmodifiableListView(_weights);
  }

  @override
  String toString() {
    return 'SplitPlanDto.weighted(weights: $weights)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$SplitPlanDto_WeightedImpl &&
            const DeepCollectionEquality().equals(other._weights, _weights));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, const DeepCollectionEquality().hash(_weights));

  /// Create a copy of SplitPlanDto
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$SplitPlanDto_WeightedImplCopyWith<_$SplitPlanDto_WeightedImpl>
  get copyWith =>
      __$$SplitPlanDto_WeightedImplCopyWithImpl<_$SplitPlanDto_WeightedImpl>(
        this,
        _$identity,
      );

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(List<String> participants) equal,
    required TResult Function(List<Payer> amounts) exact,
    required TResult Function(List<Weight> weights) weighted,
  }) {
    return weighted(weights);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(List<String> participants)? equal,
    TResult? Function(List<Payer> amounts)? exact,
    TResult? Function(List<Weight> weights)? weighted,
  }) {
    return weighted?.call(weights);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(List<String> participants)? equal,
    TResult Function(List<Payer> amounts)? exact,
    TResult Function(List<Weight> weights)? weighted,
    required TResult orElse(),
  }) {
    if (weighted != null) {
      return weighted(weights);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(SplitPlanDto_Equal value) equal,
    required TResult Function(SplitPlanDto_Exact value) exact,
    required TResult Function(SplitPlanDto_Weighted value) weighted,
  }) {
    return weighted(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(SplitPlanDto_Equal value)? equal,
    TResult? Function(SplitPlanDto_Exact value)? exact,
    TResult? Function(SplitPlanDto_Weighted value)? weighted,
  }) {
    return weighted?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(SplitPlanDto_Equal value)? equal,
    TResult Function(SplitPlanDto_Exact value)? exact,
    TResult Function(SplitPlanDto_Weighted value)? weighted,
    required TResult orElse(),
  }) {
    if (weighted != null) {
      return weighted(this);
    }
    return orElse();
  }
}

abstract class SplitPlanDto_Weighted extends SplitPlanDto {
  const factory SplitPlanDto_Weighted({required final List<Weight> weights}) =
      _$SplitPlanDto_WeightedImpl;
  const SplitPlanDto_Weighted._() : super._();

  List<Weight> get weights;

  /// Create a copy of SplitPlanDto
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$SplitPlanDto_WeightedImplCopyWith<_$SplitPlanDto_WeightedImpl>
  get copyWith => throw _privateConstructorUsedError;
}
