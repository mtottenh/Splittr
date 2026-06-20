import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/domain/models/enums.dart';
import 'package:splittr/domain/models/split.dart';
import 'package:splittr/domain/services/split_calculator.dart';

void main() {
  int sumOf(Iterable<Split> splits) =>
      splits.fold(0, (a, s) => a + s.owedCents);

  group('equal split', () {
    test('divides evenly when it divides cleanly', () {
      final splits =
          SplitCalculator.equal(totalCents: 900, userIds: ['a', 'b', 'c']);
      expect(splits.map((s) => s.owedCents), [300, 300, 300]);
    });

    test('distributes remainder cents to the first participants', () {
      final splits =
          SplitCalculator.equal(totalCents: 1000, userIds: ['a', 'b', 'c']);
      // 10.00 / 3 = 3.34, 3.33, 3.33
      expect(splits.map((s) => s.owedCents), [334, 333, 333]);
      expect(sumOf(splits), 1000);
    });

    test('single participant owes the whole amount', () {
      final splits = SplitCalculator.equal(totalCents: 555, userIds: ['a']);
      expect(splits.single.owedCents, 555);
    });

    test('throws when there are no participants', () {
      expect(
        () => SplitCalculator.equal(totalCents: 100, userIds: []),
        throwsA(isA<SplitValidationException>()),
      );
    });
  });

  group('exact split', () {
    test('accepts amounts that sum to the total', () {
      final splits = SplitCalculator.exact(
        totalCents: 1000,
        exactCents: {'a': 600, 'b': 400},
      );
      expect(sumOf(splits), 1000);
    });

    test('rejects amounts that do not sum to the total', () {
      expect(
        () => SplitCalculator.exact(
          totalCents: 1000,
          exactCents: {'a': 600, 'b': 300},
        ),
        throwsA(isA<SplitValidationException>()),
      );
    });
  });

  group('percentage split', () {
    test('splits 100% across participants exactly', () {
      final splits = SplitCalculator.percentage(
        totalCents: 1000,
        percentages: {'a': 50, 'b': 50},
      );
      expect(splits.map((s) => s.owedCents), [500, 500]);
    });

    test('handles uneven percentages and keeps the total exact', () {
      final splits = SplitCalculator.percentage(
        totalCents: 1000,
        percentages: {'a': 33.33, 'b': 33.33, 'c': 33.34},
      );
      expect(sumOf(splits), 1000);
    });

    test('rejects percentages that do not total 100', () {
      expect(
        () => SplitCalculator.percentage(
          totalCents: 1000,
          percentages: {'a': 50, 'b': 30},
        ),
        throwsA(isA<SplitValidationException>()),
      );
    });
  });

  group('shares split', () {
    test('splits proportionally to shares', () {
      final splits = SplitCalculator.shares(
        totalCents: 1000,
        shares: {'a': 3, 'b': 1},
      );
      expect(splits.firstWhere((s) => s.userId == 'a').owedCents, 750);
      expect(splits.firstWhere((s) => s.userId == 'b').owedCents, 250);
    });

    test('distributes remainder fairly and keeps the total exact', () {
      final splits = SplitCalculator.shares(
        totalCents: 1000,
        shares: {'a': 1, 'b': 1, 'c': 1},
      );
      expect(sumOf(splits), 1000);
      expect(splits.map((s) => s.owedCents), [334, 333, 333]);
    });

    test('rejects all-zero shares', () {
      expect(
        () => SplitCalculator.shares(
          totalCents: 1000,
          shares: {'a': 0, 'b': 0},
        ),
        throwsA(isA<SplitValidationException>()),
      );
    });
  });

  group('build dispatcher', () {
    test('routes to the correct strategy', () {
      final splits = SplitCalculator.build(
        type: SplitType.equal,
        totalCents: 600,
        participantIds: ['a', 'b'],
      );
      expect(splits.map((s) => s.owedCents), [300, 300]);
    });
  });
}
