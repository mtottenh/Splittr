import 'package:flutter/foundation.dart';

/// A shared ledger between a set of members (a trip, household, etc.).
@immutable
class Group {
  const Group({
    required this.id,
    required this.name,
    required this.memberIds,
    this.currencyCode = 'USD',
    this.emoji = '🧾',
    this.simplifyDebts = true,
    required this.createdAt,
  });

  final String id;
  final String name;

  /// Ordered list of member [AppUser] ids.
  final List<String> memberIds;

  final String currencyCode;
  final String emoji;

  /// When true the "settle up" suggestions minimise the number of payments by
  /// netting debts across the whole group rather than keeping them pairwise.
  final bool simplifyDebts;

  final DateTime createdAt;

  Group copyWith({
    String? name,
    List<String>? memberIds,
    String? currencyCode,
    String? emoji,
    bool? simplifyDebts,
  }) =>
      Group(
        id: id,
        name: name ?? this.name,
        memberIds: memberIds ?? this.memberIds,
        currencyCode: currencyCode ?? this.currencyCode,
        emoji: emoji ?? this.emoji,
        simplifyDebts: simplifyDebts ?? this.simplifyDebts,
        createdAt: createdAt,
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'name': name,
        'memberIds': memberIds,
        'currencyCode': currencyCode,
        'emoji': emoji,
        'simplifyDebts': simplifyDebts,
        'createdAt': createdAt.toIso8601String(),
      };

  factory Group.fromJson(Map<String, dynamic> json) => Group(
        id: json['id'] as String,
        name: json['name'] as String,
        memberIds:
            (json['memberIds'] as List<dynamic>).map((e) => e as String).toList(),
        currencyCode: json['currencyCode'] as String? ?? 'USD',
        emoji: json['emoji'] as String? ?? '🧾',
        simplifyDebts: json['simplifyDebts'] as bool? ?? true,
        createdAt: DateTime.parse(json['createdAt'] as String),
      );

  @override
  bool operator ==(Object other) =>
      other is Group &&
      other.id == id &&
      other.name == name &&
      listEquals(other.memberIds, memberIds) &&
      other.currencyCode == currencyCode &&
      other.emoji == emoji &&
      other.simplifyDebts == simplifyDebts &&
      other.createdAt == createdAt;

  @override
  int get hashCode => Object.hash(id, name, Object.hashAll(memberIds),
      currencyCode, emoji, simplifyDebts, createdAt);
}
