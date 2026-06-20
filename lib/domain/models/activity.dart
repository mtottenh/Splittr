import 'package:flutter/foundation.dart';

import 'enums.dart';

/// An immutable record of something that happened, shown in the activity feed.
@immutable
class Activity {
  const Activity({
    required this.id,
    required this.type,
    required this.actorId,
    required this.summary,
    required this.timestamp,
    this.groupId,
    this.expenseId,
    this.amountCents,
    this.currencyCode,
  });

  final String id;
  final ActivityType type;

  /// The user who performed the action.
  final String actorId;

  /// Human-readable description, pre-rendered when the event is recorded.
  final String summary;

  final DateTime timestamp;
  final String? groupId;
  final String? expenseId;
  final int? amountCents;
  final String? currencyCode;

  Map<String, dynamic> toJson() => {
        'id': id,
        'type': type.storageKey,
        'actorId': actorId,
        'summary': summary,
        'timestamp': timestamp.toIso8601String(),
        'groupId': groupId,
        'expenseId': expenseId,
        'amountCents': amountCents,
        'currencyCode': currencyCode,
      };

  factory Activity.fromJson(Map<String, dynamic> json) => Activity(
        id: json['id'] as String,
        type: ActivityTypeX.fromStorage(json['type'] as String),
        actorId: json['actorId'] as String,
        summary: json['summary'] as String,
        timestamp: DateTime.parse(json['timestamp'] as String),
        groupId: json['groupId'] as String?,
        expenseId: json['expenseId'] as String?,
        amountCents: json['amountCents'] as int?,
        currencyCode: json['currencyCode'] as String?,
      );

  @override
  bool operator ==(Object other) => other is Activity && other.id == id;

  @override
  int get hashCode => id.hashCode;
}
