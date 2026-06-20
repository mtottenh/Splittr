import 'package:flutter/foundation.dart';

/// A cash payment from one member to another that pays down a debt.
@immutable
class Settlement {
  const Settlement({
    required this.id,
    required this.groupId,
    required this.fromUserId,
    required this.toUserId,
    required this.amountCents,
    required this.currencyCode,
    required this.date,
    this.note,
  });

  final String id;

  /// Owning group id, or `null` for a one-to-one (friend) settlement.
  final String? groupId;

  /// The payer (their debt decreases).
  final String fromUserId;

  /// The recipient (the amount others owe them decreases).
  final String toUserId;

  final int amountCents;
  final String currencyCode;
  final DateTime date;
  final String? note;

  Map<String, dynamic> toJson() => {
        'id': id,
        'groupId': groupId,
        'fromUserId': fromUserId,
        'toUserId': toUserId,
        'amountCents': amountCents,
        'currencyCode': currencyCode,
        'date': date.toIso8601String(),
        'note': note,
      };

  factory Settlement.fromJson(Map<String, dynamic> json) => Settlement(
        id: json['id'] as String,
        groupId: json['groupId'] as String?,
        fromUserId: json['fromUserId'] as String,
        toUserId: json['toUserId'] as String,
        amountCents: json['amountCents'] as int,
        currencyCode: json['currencyCode'] as String? ?? 'USD',
        date: DateTime.parse(json['date'] as String),
        note: json['note'] as String?,
      );

  @override
  bool operator ==(Object other) =>
      other is Settlement &&
      other.id == id &&
      other.groupId == groupId &&
      other.fromUserId == fromUserId &&
      other.toUserId == toUserId &&
      other.amountCents == amountCents &&
      other.currencyCode == currencyCode &&
      other.date == date &&
      other.note == note;

  @override
  int get hashCode => Object.hash(id, groupId, fromUserId, toUserId,
      amountCents, currencyCode, date, note);
}
