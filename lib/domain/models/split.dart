import 'package:flutter/foundation.dart';

/// A single participant's share of an expense, expressed as the amount they
/// *owe* in integer cents.
@immutable
class Split {
  const Split({required this.userId, required this.owedCents});

  final String userId;
  final int owedCents;

  Split copyWith({int? owedCents}) =>
      Split(userId: userId, owedCents: owedCents ?? this.owedCents);

  Map<String, dynamic> toJson() => {'userId': userId, 'owedCents': owedCents};

  factory Split.fromJson(Map<String, dynamic> json) => Split(
        userId: json['userId'] as String,
        owedCents: json['owedCents'] as int,
      );

  @override
  bool operator ==(Object other) =>
      other is Split && other.userId == userId && other.owedCents == owedCents;

  @override
  int get hashCode => Object.hash(userId, owedCents);
}
