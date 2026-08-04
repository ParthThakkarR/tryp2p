// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'api.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;

/// @nodoc
mixin _$FrbTransferEvent {
  String get requestId;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $FrbTransferEventCopyWith<FrbTransferEvent> get copyWith =>
      _$FrbTransferEventCopyWithImpl<FrbTransferEvent>(
          this as FrbTransferEvent, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is FrbTransferEvent &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId));
  }

  @override
  int get hashCode => Object.hash(runtimeType, requestId);

  @override
  String toString() {
    return 'FrbTransferEvent(requestId: $requestId)';
  }
}

/// @nodoc
abstract mixin class $FrbTransferEventCopyWith<$Res> {
  factory $FrbTransferEventCopyWith(
          FrbTransferEvent value, $Res Function(FrbTransferEvent) _then) =
      _$FrbTransferEventCopyWithImpl;
  @useResult
  $Res call({String requestId});
}

/// @nodoc
class _$FrbTransferEventCopyWithImpl<$Res>
    implements $FrbTransferEventCopyWith<$Res> {
  _$FrbTransferEventCopyWithImpl(this._self, this._then);

  final FrbTransferEvent _self;
  final $Res Function(FrbTransferEvent) _then;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? requestId = null,
  }) {
    return _then(_self.copyWith(
      requestId: null == requestId
          ? _self.requestId
          : requestId // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// Adds pattern-matching-related methods to [FrbTransferEvent].
extension FrbTransferEventPatterns on FrbTransferEvent {
  /// A variant of `map` that fallback to returning `orElse`.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case _:
  ///     return orElse();
  /// }
  /// ```

  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(FrbTransferEvent_SendStatus value)? sendStatus,
    TResult Function(FrbTransferEvent_Rejected value)? rejected,
    TResult Function(FrbTransferEvent_Error value)? error,
    TResult Function(FrbTransferEvent_Incoming value)? incoming,
    TResult Function(FrbTransferEvent_Progress value)? progress,
    TResult Function(FrbTransferEvent_Complete value)? complete,
    TResult Function(FrbTransferEvent_Cancelled value)? cancelled,
    required TResult orElse(),
  }) {
    final _that = this;
    switch (_that) {
      case FrbTransferEvent_SendStatus() when sendStatus != null:
        return sendStatus(_that);
      case FrbTransferEvent_Rejected() when rejected != null:
        return rejected(_that);
      case FrbTransferEvent_Error() when error != null:
        return error(_that);
      case FrbTransferEvent_Incoming() when incoming != null:
        return incoming(_that);
      case FrbTransferEvent_Progress() when progress != null:
        return progress(_that);
      case FrbTransferEvent_Complete() when complete != null:
        return complete(_that);
      case FrbTransferEvent_Cancelled() when cancelled != null:
        return cancelled(_that);
      case _:
        return orElse();
    }
  }

  /// A `switch`-like method, using callbacks.
  ///
  /// Callbacks receives the raw object, upcasted.
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case final Subclass2 value:
  ///     return ...;
  /// }
  /// ```

  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(FrbTransferEvent_SendStatus value) sendStatus,
    required TResult Function(FrbTransferEvent_Rejected value) rejected,
    required TResult Function(FrbTransferEvent_Error value) error,
    required TResult Function(FrbTransferEvent_Incoming value) incoming,
    required TResult Function(FrbTransferEvent_Progress value) progress,
    required TResult Function(FrbTransferEvent_Complete value) complete,
    required TResult Function(FrbTransferEvent_Cancelled value) cancelled,
  }) {
    final _that = this;
    switch (_that) {
      case FrbTransferEvent_SendStatus():
        return sendStatus(_that);
      case FrbTransferEvent_Rejected():
        return rejected(_that);
      case FrbTransferEvent_Error():
        return error(_that);
      case FrbTransferEvent_Incoming():
        return incoming(_that);
      case FrbTransferEvent_Progress():
        return progress(_that);
      case FrbTransferEvent_Complete():
        return complete(_that);
      case FrbTransferEvent_Cancelled():
        return cancelled(_that);
    }
  }

  /// A variant of `map` that fallback to returning `null`.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case final Subclass value:
  ///     return ...;
  ///   case _:
  ///     return null;
  /// }
  /// ```

  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(FrbTransferEvent_SendStatus value)? sendStatus,
    TResult? Function(FrbTransferEvent_Rejected value)? rejected,
    TResult? Function(FrbTransferEvent_Error value)? error,
    TResult? Function(FrbTransferEvent_Incoming value)? incoming,
    TResult? Function(FrbTransferEvent_Progress value)? progress,
    TResult? Function(FrbTransferEvent_Complete value)? complete,
    TResult? Function(FrbTransferEvent_Cancelled value)? cancelled,
  }) {
    final _that = this;
    switch (_that) {
      case FrbTransferEvent_SendStatus() when sendStatus != null:
        return sendStatus(_that);
      case FrbTransferEvent_Rejected() when rejected != null:
        return rejected(_that);
      case FrbTransferEvent_Error() when error != null:
        return error(_that);
      case FrbTransferEvent_Incoming() when incoming != null:
        return incoming(_that);
      case FrbTransferEvent_Progress() when progress != null:
        return progress(_that);
      case FrbTransferEvent_Complete() when complete != null:
        return complete(_that);
      case FrbTransferEvent_Cancelled() when cancelled != null:
        return cancelled(_that);
      case _:
        return null;
    }
  }

  /// A variant of `when` that fallback to an `orElse` callback.
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case _:
  ///     return orElse();
  /// }
  /// ```

  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(String requestId, String status)? sendStatus,
    TResult Function(String requestId)? rejected,
    TResult Function(String requestId, String error)? error,
    TResult Function(String requestId, String senderName, String senderNodeId,
            String fileName, BigInt fileSize)?
        incoming,
    TResult Function(String requestId, BigInt bytesTransferred, BigInt total,
            double speedBytesPerSec)?
        progress,
    TResult Function(String requestId, String filePath, String blake3Hash,
            double elapsedSecs)?
        complete,
    TResult Function(String requestId)? cancelled,
    required TResult orElse(),
  }) {
    final _that = this;
    switch (_that) {
      case FrbTransferEvent_SendStatus() when sendStatus != null:
        return sendStatus(_that.requestId, _that.status);
      case FrbTransferEvent_Rejected() when rejected != null:
        return rejected(_that.requestId);
      case FrbTransferEvent_Error() when error != null:
        return error(_that.requestId, _that.error);
      case FrbTransferEvent_Incoming() when incoming != null:
        return incoming(_that.requestId, _that.senderName, _that.senderNodeId,
            _that.fileName, _that.fileSize);
      case FrbTransferEvent_Progress() when progress != null:
        return progress(_that.requestId, _that.bytesTransferred, _that.total,
            _that.speedBytesPerSec);
      case FrbTransferEvent_Complete() when complete != null:
        return complete(_that.requestId, _that.filePath, _that.blake3Hash,
            _that.elapsedSecs);
      case FrbTransferEvent_Cancelled() when cancelled != null:
        return cancelled(_that.requestId);
      case _:
        return orElse();
    }
  }

  /// A `switch`-like method, using callbacks.
  ///
  /// As opposed to `map`, this offers destructuring.
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case Subclass2(:final field2):
  ///     return ...;
  /// }
  /// ```

  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(String requestId, String status) sendStatus,
    required TResult Function(String requestId) rejected,
    required TResult Function(String requestId, String error) error,
    required TResult Function(String requestId, String senderName,
            String senderNodeId, String fileName, BigInt fileSize)
        incoming,
    required TResult Function(String requestId, BigInt bytesTransferred,
            BigInt total, double speedBytesPerSec)
        progress,
    required TResult Function(String requestId, String filePath,
            String blake3Hash, double elapsedSecs)
        complete,
    required TResult Function(String requestId) cancelled,
  }) {
    final _that = this;
    switch (_that) {
      case FrbTransferEvent_SendStatus():
        return sendStatus(_that.requestId, _that.status);
      case FrbTransferEvent_Rejected():
        return rejected(_that.requestId);
      case FrbTransferEvent_Error():
        return error(_that.requestId, _that.error);
      case FrbTransferEvent_Incoming():
        return incoming(_that.requestId, _that.senderName, _that.senderNodeId,
            _that.fileName, _that.fileSize);
      case FrbTransferEvent_Progress():
        return progress(_that.requestId, _that.bytesTransferred, _that.total,
            _that.speedBytesPerSec);
      case FrbTransferEvent_Complete():
        return complete(_that.requestId, _that.filePath, _that.blake3Hash,
            _that.elapsedSecs);
      case FrbTransferEvent_Cancelled():
        return cancelled(_that.requestId);
    }
  }

  /// A variant of `when` that fallback to returning `null`
  ///
  /// It is equivalent to doing:
  /// ```dart
  /// switch (sealedClass) {
  ///   case Subclass(:final field):
  ///     return ...;
  ///   case _:
  ///     return null;
  /// }
  /// ```

  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(String requestId, String status)? sendStatus,
    TResult? Function(String requestId)? rejected,
    TResult? Function(String requestId, String error)? error,
    TResult? Function(String requestId, String senderName, String senderNodeId,
            String fileName, BigInt fileSize)?
        incoming,
    TResult? Function(String requestId, BigInt bytesTransferred, BigInt total,
            double speedBytesPerSec)?
        progress,
    TResult? Function(String requestId, String filePath, String blake3Hash,
            double elapsedSecs)?
        complete,
    TResult? Function(String requestId)? cancelled,
  }) {
    final _that = this;
    switch (_that) {
      case FrbTransferEvent_SendStatus() when sendStatus != null:
        return sendStatus(_that.requestId, _that.status);
      case FrbTransferEvent_Rejected() when rejected != null:
        return rejected(_that.requestId);
      case FrbTransferEvent_Error() when error != null:
        return error(_that.requestId, _that.error);
      case FrbTransferEvent_Incoming() when incoming != null:
        return incoming(_that.requestId, _that.senderName, _that.senderNodeId,
            _that.fileName, _that.fileSize);
      case FrbTransferEvent_Progress() when progress != null:
        return progress(_that.requestId, _that.bytesTransferred, _that.total,
            _that.speedBytesPerSec);
      case FrbTransferEvent_Complete() when complete != null:
        return complete(_that.requestId, _that.filePath, _that.blake3Hash,
            _that.elapsedSecs);
      case FrbTransferEvent_Cancelled() when cancelled != null:
        return cancelled(_that.requestId);
      case _:
        return null;
    }
  }
}

/// @nodoc

class FrbTransferEvent_SendStatus extends FrbTransferEvent {
  const FrbTransferEvent_SendStatus(
      {required this.requestId, required this.status})
      : super._();

  @override
  final String requestId;
  final String status;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $FrbTransferEvent_SendStatusCopyWith<FrbTransferEvent_SendStatus>
      get copyWith => _$FrbTransferEvent_SendStatusCopyWithImpl<
          FrbTransferEvent_SendStatus>(this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is FrbTransferEvent_SendStatus &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId) &&
            (identical(other.status, status) || other.status == status));
  }

  @override
  int get hashCode => Object.hash(runtimeType, requestId, status);

  @override
  String toString() {
    return 'FrbTransferEvent.sendStatus(requestId: $requestId, status: $status)';
  }
}

/// @nodoc
abstract mixin class $FrbTransferEvent_SendStatusCopyWith<$Res>
    implements $FrbTransferEventCopyWith<$Res> {
  factory $FrbTransferEvent_SendStatusCopyWith(
          FrbTransferEvent_SendStatus value,
          $Res Function(FrbTransferEvent_SendStatus) _then) =
      _$FrbTransferEvent_SendStatusCopyWithImpl;
  @override
  @useResult
  $Res call({String requestId, String status});
}

/// @nodoc
class _$FrbTransferEvent_SendStatusCopyWithImpl<$Res>
    implements $FrbTransferEvent_SendStatusCopyWith<$Res> {
  _$FrbTransferEvent_SendStatusCopyWithImpl(this._self, this._then);

  final FrbTransferEvent_SendStatus _self;
  final $Res Function(FrbTransferEvent_SendStatus) _then;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $Res call({
    Object? requestId = null,
    Object? status = null,
  }) {
    return _then(FrbTransferEvent_SendStatus(
      requestId: null == requestId
          ? _self.requestId
          : requestId // ignore: cast_nullable_to_non_nullable
              as String,
      status: null == status
          ? _self.status
          : status // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// @nodoc

class FrbTransferEvent_Rejected extends FrbTransferEvent {
  const FrbTransferEvent_Rejected({required this.requestId}) : super._();

  @override
  final String requestId;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $FrbTransferEvent_RejectedCopyWith<FrbTransferEvent_Rejected> get copyWith =>
      _$FrbTransferEvent_RejectedCopyWithImpl<FrbTransferEvent_Rejected>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is FrbTransferEvent_Rejected &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId));
  }

  @override
  int get hashCode => Object.hash(runtimeType, requestId);

  @override
  String toString() {
    return 'FrbTransferEvent.rejected(requestId: $requestId)';
  }
}

/// @nodoc
abstract mixin class $FrbTransferEvent_RejectedCopyWith<$Res>
    implements $FrbTransferEventCopyWith<$Res> {
  factory $FrbTransferEvent_RejectedCopyWith(FrbTransferEvent_Rejected value,
          $Res Function(FrbTransferEvent_Rejected) _then) =
      _$FrbTransferEvent_RejectedCopyWithImpl;
  @override
  @useResult
  $Res call({String requestId});
}

/// @nodoc
class _$FrbTransferEvent_RejectedCopyWithImpl<$Res>
    implements $FrbTransferEvent_RejectedCopyWith<$Res> {
  _$FrbTransferEvent_RejectedCopyWithImpl(this._self, this._then);

  final FrbTransferEvent_Rejected _self;
  final $Res Function(FrbTransferEvent_Rejected) _then;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $Res call({
    Object? requestId = null,
  }) {
    return _then(FrbTransferEvent_Rejected(
      requestId: null == requestId
          ? _self.requestId
          : requestId // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// @nodoc

class FrbTransferEvent_Error extends FrbTransferEvent {
  const FrbTransferEvent_Error({required this.requestId, required this.error})
      : super._();

  @override
  final String requestId;
  final String error;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $FrbTransferEvent_ErrorCopyWith<FrbTransferEvent_Error> get copyWith =>
      _$FrbTransferEvent_ErrorCopyWithImpl<FrbTransferEvent_Error>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is FrbTransferEvent_Error &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId) &&
            (identical(other.error, error) || other.error == error));
  }

  @override
  int get hashCode => Object.hash(runtimeType, requestId, error);

  @override
  String toString() {
    return 'FrbTransferEvent.error(requestId: $requestId, error: $error)';
  }
}

/// @nodoc
abstract mixin class $FrbTransferEvent_ErrorCopyWith<$Res>
    implements $FrbTransferEventCopyWith<$Res> {
  factory $FrbTransferEvent_ErrorCopyWith(FrbTransferEvent_Error value,
          $Res Function(FrbTransferEvent_Error) _then) =
      _$FrbTransferEvent_ErrorCopyWithImpl;
  @override
  @useResult
  $Res call({String requestId, String error});
}

/// @nodoc
class _$FrbTransferEvent_ErrorCopyWithImpl<$Res>
    implements $FrbTransferEvent_ErrorCopyWith<$Res> {
  _$FrbTransferEvent_ErrorCopyWithImpl(this._self, this._then);

  final FrbTransferEvent_Error _self;
  final $Res Function(FrbTransferEvent_Error) _then;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $Res call({
    Object? requestId = null,
    Object? error = null,
  }) {
    return _then(FrbTransferEvent_Error(
      requestId: null == requestId
          ? _self.requestId
          : requestId // ignore: cast_nullable_to_non_nullable
              as String,
      error: null == error
          ? _self.error
          : error // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// @nodoc

class FrbTransferEvent_Incoming extends FrbTransferEvent {
  const FrbTransferEvent_Incoming(
      {required this.requestId,
      required this.senderName,
      required this.senderNodeId,
      required this.fileName,
      required this.fileSize})
      : super._();

  @override
  final String requestId;
  final String senderName;
  final String senderNodeId;
  final String fileName;
  final BigInt fileSize;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $FrbTransferEvent_IncomingCopyWith<FrbTransferEvent_Incoming> get copyWith =>
      _$FrbTransferEvent_IncomingCopyWithImpl<FrbTransferEvent_Incoming>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is FrbTransferEvent_Incoming &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId) &&
            (identical(other.senderName, senderName) ||
                other.senderName == senderName) &&
            (identical(other.senderNodeId, senderNodeId) ||
                other.senderNodeId == senderNodeId) &&
            (identical(other.fileName, fileName) ||
                other.fileName == fileName) &&
            (identical(other.fileSize, fileSize) ||
                other.fileSize == fileSize));
  }

  @override
  int get hashCode => Object.hash(
      runtimeType, requestId, senderName, senderNodeId, fileName, fileSize);

  @override
  String toString() {
    return 'FrbTransferEvent.incoming(requestId: $requestId, senderName: $senderName, senderNodeId: $senderNodeId, fileName: $fileName, fileSize: $fileSize)';
  }
}

/// @nodoc
abstract mixin class $FrbTransferEvent_IncomingCopyWith<$Res>
    implements $FrbTransferEventCopyWith<$Res> {
  factory $FrbTransferEvent_IncomingCopyWith(FrbTransferEvent_Incoming value,
          $Res Function(FrbTransferEvent_Incoming) _then) =
      _$FrbTransferEvent_IncomingCopyWithImpl;
  @override
  @useResult
  $Res call(
      {String requestId,
      String senderName,
      String senderNodeId,
      String fileName,
      BigInt fileSize});
}

/// @nodoc
class _$FrbTransferEvent_IncomingCopyWithImpl<$Res>
    implements $FrbTransferEvent_IncomingCopyWith<$Res> {
  _$FrbTransferEvent_IncomingCopyWithImpl(this._self, this._then);

  final FrbTransferEvent_Incoming _self;
  final $Res Function(FrbTransferEvent_Incoming) _then;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $Res call({
    Object? requestId = null,
    Object? senderName = null,
    Object? senderNodeId = null,
    Object? fileName = null,
    Object? fileSize = null,
  }) {
    return _then(FrbTransferEvent_Incoming(
      requestId: null == requestId
          ? _self.requestId
          : requestId // ignore: cast_nullable_to_non_nullable
              as String,
      senderName: null == senderName
          ? _self.senderName
          : senderName // ignore: cast_nullable_to_non_nullable
              as String,
      senderNodeId: null == senderNodeId
          ? _self.senderNodeId
          : senderNodeId // ignore: cast_nullable_to_non_nullable
              as String,
      fileName: null == fileName
          ? _self.fileName
          : fileName // ignore: cast_nullable_to_non_nullable
              as String,
      fileSize: null == fileSize
          ? _self.fileSize
          : fileSize // ignore: cast_nullable_to_non_nullable
              as BigInt,
    ));
  }
}

/// @nodoc

class FrbTransferEvent_Progress extends FrbTransferEvent {
  const FrbTransferEvent_Progress(
      {required this.requestId,
      required this.bytesTransferred,
      required this.total,
      required this.speedBytesPerSec})
      : super._();

  @override
  final String requestId;
  final BigInt bytesTransferred;
  final BigInt total;
  final double speedBytesPerSec;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $FrbTransferEvent_ProgressCopyWith<FrbTransferEvent_Progress> get copyWith =>
      _$FrbTransferEvent_ProgressCopyWithImpl<FrbTransferEvent_Progress>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is FrbTransferEvent_Progress &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId) &&
            (identical(other.bytesTransferred, bytesTransferred) ||
                other.bytesTransferred == bytesTransferred) &&
            (identical(other.total, total) || other.total == total) &&
            (identical(other.speedBytesPerSec, speedBytesPerSec) ||
                other.speedBytesPerSec == speedBytesPerSec));
  }

  @override
  int get hashCode => Object.hash(
      runtimeType, requestId, bytesTransferred, total, speedBytesPerSec);

  @override
  String toString() {
    return 'FrbTransferEvent.progress(requestId: $requestId, bytesTransferred: $bytesTransferred, total: $total, speedBytesPerSec: $speedBytesPerSec)';
  }
}

/// @nodoc
abstract mixin class $FrbTransferEvent_ProgressCopyWith<$Res>
    implements $FrbTransferEventCopyWith<$Res> {
  factory $FrbTransferEvent_ProgressCopyWith(FrbTransferEvent_Progress value,
          $Res Function(FrbTransferEvent_Progress) _then) =
      _$FrbTransferEvent_ProgressCopyWithImpl;
  @override
  @useResult
  $Res call(
      {String requestId,
      BigInt bytesTransferred,
      BigInt total,
      double speedBytesPerSec});
}

/// @nodoc
class _$FrbTransferEvent_ProgressCopyWithImpl<$Res>
    implements $FrbTransferEvent_ProgressCopyWith<$Res> {
  _$FrbTransferEvent_ProgressCopyWithImpl(this._self, this._then);

  final FrbTransferEvent_Progress _self;
  final $Res Function(FrbTransferEvent_Progress) _then;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $Res call({
    Object? requestId = null,
    Object? bytesTransferred = null,
    Object? total = null,
    Object? speedBytesPerSec = null,
  }) {
    return _then(FrbTransferEvent_Progress(
      requestId: null == requestId
          ? _self.requestId
          : requestId // ignore: cast_nullable_to_non_nullable
              as String,
      bytesTransferred: null == bytesTransferred
          ? _self.bytesTransferred
          : bytesTransferred // ignore: cast_nullable_to_non_nullable
              as BigInt,
      total: null == total
          ? _self.total
          : total // ignore: cast_nullable_to_non_nullable
              as BigInt,
      speedBytesPerSec: null == speedBytesPerSec
          ? _self.speedBytesPerSec
          : speedBytesPerSec // ignore: cast_nullable_to_non_nullable
              as double,
    ));
  }
}

/// @nodoc

class FrbTransferEvent_Complete extends FrbTransferEvent {
  const FrbTransferEvent_Complete(
      {required this.requestId,
      required this.filePath,
      required this.blake3Hash,
      required this.elapsedSecs})
      : super._();

  @override
  final String requestId;
  final String filePath;
  final String blake3Hash;
  final double elapsedSecs;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $FrbTransferEvent_CompleteCopyWith<FrbTransferEvent_Complete> get copyWith =>
      _$FrbTransferEvent_CompleteCopyWithImpl<FrbTransferEvent_Complete>(
          this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is FrbTransferEvent_Complete &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId) &&
            (identical(other.filePath, filePath) ||
                other.filePath == filePath) &&
            (identical(other.blake3Hash, blake3Hash) ||
                other.blake3Hash == blake3Hash) &&
            (identical(other.elapsedSecs, elapsedSecs) ||
                other.elapsedSecs == elapsedSecs));
  }

  @override
  int get hashCode =>
      Object.hash(runtimeType, requestId, filePath, blake3Hash, elapsedSecs);

  @override
  String toString() {
    return 'FrbTransferEvent.complete(requestId: $requestId, filePath: $filePath, blake3Hash: $blake3Hash, elapsedSecs: $elapsedSecs)';
  }
}

/// @nodoc
abstract mixin class $FrbTransferEvent_CompleteCopyWith<$Res>
    implements $FrbTransferEventCopyWith<$Res> {
  factory $FrbTransferEvent_CompleteCopyWith(FrbTransferEvent_Complete value,
          $Res Function(FrbTransferEvent_Complete) _then) =
      _$FrbTransferEvent_CompleteCopyWithImpl;
  @override
  @useResult
  $Res call(
      {String requestId,
      String filePath,
      String blake3Hash,
      double elapsedSecs});
}

/// @nodoc
class _$FrbTransferEvent_CompleteCopyWithImpl<$Res>
    implements $FrbTransferEvent_CompleteCopyWith<$Res> {
  _$FrbTransferEvent_CompleteCopyWithImpl(this._self, this._then);

  final FrbTransferEvent_Complete _self;
  final $Res Function(FrbTransferEvent_Complete) _then;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $Res call({
    Object? requestId = null,
    Object? filePath = null,
    Object? blake3Hash = null,
    Object? elapsedSecs = null,
  }) {
    return _then(FrbTransferEvent_Complete(
      requestId: null == requestId
          ? _self.requestId
          : requestId // ignore: cast_nullable_to_non_nullable
              as String,
      filePath: null == filePath
          ? _self.filePath
          : filePath // ignore: cast_nullable_to_non_nullable
              as String,
      blake3Hash: null == blake3Hash
          ? _self.blake3Hash
          : blake3Hash // ignore: cast_nullable_to_non_nullable
              as String,
      elapsedSecs: null == elapsedSecs
          ? _self.elapsedSecs
          : elapsedSecs // ignore: cast_nullable_to_non_nullable
              as double,
    ));
  }
}

/// @nodoc

class FrbTransferEvent_Cancelled extends FrbTransferEvent {
  const FrbTransferEvent_Cancelled({required this.requestId}) : super._();

  @override
  final String requestId;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @JsonKey(includeFromJson: false, includeToJson: false)
  @pragma('vm:prefer-inline')
  $FrbTransferEvent_CancelledCopyWith<FrbTransferEvent_Cancelled>
      get copyWith =>
          _$FrbTransferEvent_CancelledCopyWithImpl<FrbTransferEvent_Cancelled>(
              this, _$identity);

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is FrbTransferEvent_Cancelled &&
            (identical(other.requestId, requestId) ||
                other.requestId == requestId));
  }

  @override
  int get hashCode => Object.hash(runtimeType, requestId);

  @override
  String toString() {
    return 'FrbTransferEvent.cancelled(requestId: $requestId)';
  }
}

/// @nodoc
abstract mixin class $FrbTransferEvent_CancelledCopyWith<$Res>
    implements $FrbTransferEventCopyWith<$Res> {
  factory $FrbTransferEvent_CancelledCopyWith(FrbTransferEvent_Cancelled value,
          $Res Function(FrbTransferEvent_Cancelled) _then) =
      _$FrbTransferEvent_CancelledCopyWithImpl;
  @override
  @useResult
  $Res call({String requestId});
}

/// @nodoc
class _$FrbTransferEvent_CancelledCopyWithImpl<$Res>
    implements $FrbTransferEvent_CancelledCopyWith<$Res> {
  _$FrbTransferEvent_CancelledCopyWithImpl(this._self, this._then);

  final FrbTransferEvent_Cancelled _self;
  final $Res Function(FrbTransferEvent_Cancelled) _then;

  /// Create a copy of FrbTransferEvent
  /// with the given fields replaced by the non-null parameter values.
  @override
  @pragma('vm:prefer-inline')
  $Res call({
    Object? requestId = null,
  }) {
    return _then(FrbTransferEvent_Cancelled(
      requestId: null == requestId
          ? _self.requestId
          : requestId // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

// dart format on
