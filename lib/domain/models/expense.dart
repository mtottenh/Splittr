import 'package:flutter/foundation.dart';

import 'enums.dart';
import 'split.dart';

/// A shared cost. The invariant the domain guarantees is:
///
///   sum(paidBy.values) == totalCents == sum(splits.owedCents)
///
/// i.e. the money put in equals the cost equals the money owed. The
/// [SplitCalculator] builds [splits] so this always holds.
@immutable
class Expense {
  const Expense({
    required this.id,
    required this.groupId,
    required this.description,
    required this.totalCents,
    required this.currencyCode,
    required this.paidBy,
    required this.splits,
    required this.splitType,
    required this.categoryId,
    required this.date,
    required this.createdAt,
    this.notes,
  });

  final String id;

  /// Owning group id, or `null` for a one-to-one (friend) expense.
  final String? groupId;

  final String description;
  final int totalCents;
  final String currencyCode;

  /// Map of payer user id -> amount they fronted, in cents. Supports the common
  /// single-payer case and the occasional "we both chipped in" case.
  final Map<String, int> paidBy;

  final List<Split> splits;
  final SplitType splitType;
  final String categoryId;
  final DateTime date;
  final DateTime createdAt;
  final String? notes;

  /// User ids that owe a non-zero share.
  Iterable<String> get participantIds => splits.map((s) => s.userId);

  /// The amount [userId] paid up front (0 if they paid nothing).
  int paidByUser(String userId) => paidBy[userId] ?? 0;

  /// The amount [userId] owes for this expense (0 if not a participant).
  int owedByUser(String userId) =>
      splits.where((s) => s.userId == userId).fold(0, (a, s) => a + s.owedCents);

  Expense copyWith({
    String? groupId,
    String? description,
    int? totalCents,
    String? currencyCode,
    Map<String, int>? paidBy,
    List<Split>? splits,
    SplitType? splitType,
    String? categoryId,
    DateTime? date,
    String? notes,
  }) =>
      Expense(
        id: id,
        groupId: groupId ?? this.groupId,
        description: description ?? this.description,
        totalCents: totalCents ?? this.totalCents,
        currencyCode: currencyCode ?? this.currencyCode,
        paidBy: paidBy ?? this.paidBy,
        splits: splits ?? this.splits,
        splitType: splitType ?? this.splitType,
        categoryId: categoryId ?? this.categoryId,
        date: date ?? this.date,
        createdAt: createdAt,
        notes: notes ?? this.notes,
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'groupId': groupId,
        'description': description,
        'totalCents': totalCents,
        'currencyCode': currencyCode,
        'paidBy': paidBy,
        'splits': splits.map((s) => s.toJson()).toList(),
        'splitType': splitType.storageKey,
        'categoryId': categoryId,
        'date': date.toIso8601String(),
        'createdAt': createdAt.toIso8601String(),
        'notes': notes,
      };

  factory Expense.fromJson(Map<String, dynamic> json) => Expense(
        id: json['id'] as String,
        groupId: json['groupId'] as String?,
        description: json['description'] as String,
        totalCents: json['totalCents'] as int,
        currencyCode: json['currencyCode'] as String? ?? 'USD',
        paidBy: (json['paidBy'] as Map<String, dynamic>)
            .map((k, v) => MapEntry(k, v as int)),
        splits: (json['splits'] as List<dynamic>)
            .map((e) => Split.fromJson(e as Map<String, dynamic>))
            .toList(),
        splitType: SplitTypeX.fromStorage(json['splitType'] as String),
        categoryId: json['categoryId'] as String? ?? 'general',
        date: DateTime.parse(json['date'] as String),
        createdAt: DateTime.parse(json['createdAt'] as String),
        notes: json['notes'] as String?,
      );

  @override
  bool operator ==(Object other) =>
      other is Expense &&
      other.id == id &&
      other.groupId == groupId &&
      other.description == description &&
      other.totalCents == totalCents &&
      other.currencyCode == currencyCode &&
      mapEquals(other.paidBy, paidBy) &&
      listEquals(other.splits, splits) &&
      other.splitType == splitType &&
      other.categoryId == categoryId &&
      other.date == date &&
      other.notes == notes;

  @override
  int get hashCode => Object.hash(
        id,
        groupId,
        description,
        totalCents,
        currencyCode,
        Object.hashAll(paidBy.entries.map((e) => Object.hash(e.key, e.value))),
        Object.hashAll(splits),
        splitType,
        categoryId,
        date,
        notes,
      );
}
