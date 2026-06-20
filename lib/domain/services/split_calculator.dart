import '../models/enums.dart';
import '../models/split.dart';

/// Thrown when the inputs to a split cannot produce a valid result, e.g. exact
/// amounts that don't add up to the total.
class SplitValidationException implements Exception {
  const SplitValidationException(this.message);
  final String message;
  @override
  String toString() => 'SplitValidationException: $message';
}

/// Pure functions that turn a total and a chosen strategy into per-user
/// [Split]s whose owed amounts always sum *exactly* to the total.
///
/// All arithmetic is in integer cents. Remainder cents (which appear when a
/// total doesn't divide evenly) are distributed using the **largest-remainder
/// method** so the result is both exact and as fair as possible.
class SplitCalculator {
  const SplitCalculator._();

  /// Splits [totalCents] equally between [userIds].
  ///
  /// The first `r` users (where `r` is the leftover cents) each receive one
  /// extra cent, so e.g. \$10.00 between 3 people becomes 3.34 / 3.33 / 3.33.
  static List<Split> equal({
    required int totalCents,
    required List<String> userIds,
  }) {
    _requireParticipants(userIds);
    final base = totalCents ~/ userIds.length;
    final remainder = totalCents - base * userIds.length;
    return [
      for (var i = 0; i < userIds.length; i++)
        Split(
          userId: userIds[i],
          owedCents: base + (i < remainder ? 1 : 0),
        ),
    ];
  }

  /// Uses the [exactCents] amounts entered per user verbatim, after validating
  /// they sum to [totalCents].
  static List<Split> exact({
    required int totalCents,
    required Map<String, int> exactCents,
  }) {
    _requireParticipants(exactCents.keys.toList());
    final sum = exactCents.values.fold(0, (a, b) => a + b);
    if (sum != totalCents) {
      throw SplitValidationException(
        'Exact amounts add up to $sum¢ but the total is $totalCents¢.',
      );
    }
    return [
      for (final entry in exactCents.entries)
        Split(userId: entry.key, owedCents: entry.value),
    ];
  }

  /// Splits [totalCents] by [percentages] (which must sum to 100).
  static List<Split> percentage({
    required int totalCents,
    required Map<String, double> percentages,
  }) {
    _requireParticipants(percentages.keys.toList());
    final sum = percentages.values.fold(0.0, (a, b) => a + b);
    if ((sum - 100).abs() > 0.01) {
      throw SplitValidationException(
        'Percentages add up to ${sum.toStringAsFixed(2)}% but must total 100%.',
      );
    }
    final weights =
        percentages.map((user, pct) => MapEntry(user, pct));
    return _byWeights(totalCents: totalCents, weights: weights);
  }

  /// Splits [totalCents] proportionally to integer [shares] (e.g. 2:1:1).
  static List<Split> shares({
    required int totalCents,
    required Map<String, int> shares,
  }) {
    _requireParticipants(shares.keys.toList());
    if (shares.values.any((s) => s < 0)) {
      throw const SplitValidationException('Shares cannot be negative.');
    }
    if (shares.values.fold(0, (a, b) => a + b) == 0) {
      throw const SplitValidationException('At least one share is required.');
    }
    final weights = shares.map((user, s) => MapEntry(user, s.toDouble()));
    return _byWeights(totalCents: totalCents, weights: weights);
  }

  /// Generic weighted distribution shared by [percentage] and [shares].
  ///
  /// Assigns each user `floor(total * weight / totalWeight)` cents, then hands
  /// the leftover cents one-by-one to the users whose fractional remainder was
  /// largest. This guarantees `sum(result) == totalCents`.
  static List<Split> _byWeights({
    required int totalCents,
    required Map<String, double> weights,
  }) {
    final totalWeight = weights.values.fold(0.0, (a, b) => a + b);
    final ids = weights.keys.toList();

    final exactShares = <String, double>{
      for (final id in ids) id: totalCents * weights[id]! / totalWeight,
    };
    final floored = <String, int>{
      for (final id in ids) id: exactShares[id]!.floor(),
    };
    final distributed = floored.values.fold(0, (a, b) => a + b);
    var leftover = totalCents - distributed;

    // Order by descending fractional remainder, tie-broken by insertion order.
    final byRemainder = ids.toList()
      ..sort((a, b) {
        final fa = exactShares[a]! - floored[a]!;
        final fb = exactShares[b]! - floored[b]!;
        final cmp = fb.compareTo(fa);
        return cmp != 0 ? cmp : ids.indexOf(a).compareTo(ids.indexOf(b));
      });

    final result = Map<String, int>.from(floored);
    var i = 0;
    while (leftover > 0) {
      result[byRemainder[i % byRemainder.length]] =
          result[byRemainder[i % byRemainder.length]]! + 1;
      leftover--;
      i++;
    }

    return [
      for (final id in ids) Split(userId: id, owedCents: result[id]!),
    ];
  }

  static void _requireParticipants(List<String> ids) {
    if (ids.isEmpty) {
      throw const SplitValidationException(
          'An expense needs at least one participant.');
    }
  }

  /// Convenience dispatcher used by the UI: builds splits from whichever raw
  /// input matches [type].
  static List<Split> build({
    required SplitType type,
    required int totalCents,
    required List<String> participantIds,
    Map<String, int>? exactCents,
    Map<String, double>? percentages,
    Map<String, int>? shares,
  }) {
    switch (type) {
      case SplitType.equal:
        return equal(totalCents: totalCents, userIds: participantIds);
      case SplitType.exact:
        return exact(
          totalCents: totalCents,
          exactCents: exactCents ?? const {},
        );
      case SplitType.percentage:
        return percentage(
          totalCents: totalCents,
          percentages: percentages ?? const {},
        );
      case SplitType.shares:
        return SplitCalculator.shares(
          totalCents: totalCents,
          shares: shares ?? const {},
        );
    }
  }
}
