import 'package:flutter/foundation.dart';

/// A user's net position: positive means the group owes them money, negative
/// means they owe the group. These are *computed* values, never persisted.
@immutable
class Balance {
  const Balance({required this.userId, required this.netCents});

  final String userId;

  /// (total paid) - (total owed) across expenses and settlements, in cents.
  final int netCents;

  bool get isCreditor => netCents > 0;
  bool get isDebtor => netCents < 0;
  bool get isSettled => netCents == 0;

  @override
  bool operator ==(Object other) =>
      other is Balance && other.userId == userId && other.netCents == netCents;

  @override
  int get hashCode => Object.hash(userId, netCents);
}

/// A directed "who pays whom" edge: [fromUserId] owes [toUserId] [amountCents].
@immutable
class DebtEdge {
  const DebtEdge({
    required this.fromUserId,
    required this.toUserId,
    required this.amountCents,
  });

  final String fromUserId;
  final String toUserId;
  final int amountCents;

  @override
  bool operator ==(Object other) =>
      other is DebtEdge &&
      other.fromUserId == fromUserId &&
      other.toUserId == toUserId &&
      other.amountCents == amountCents;

  @override
  int get hashCode => Object.hash(fromUserId, toUserId, amountCents);

  @override
  String toString() => '$fromUserId -> $toUserId : $amountCents';
}
