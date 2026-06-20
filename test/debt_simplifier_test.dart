import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/domain/services/debt_simplifier.dart';

void main() {
  group('DebtSimplifier', () {
    test('returns no payments when everyone is settled', () {
      expect(DebtSimplifier.minimize({'a': 0, 'b': 0}), isEmpty);
    });

    test('produces a single payment for a simple two-person debt', () {
      final edges = DebtSimplifier.minimize({'a': -500, 'b': 500});
      expect(edges, hasLength(1));
      expect(edges.single.fromUserId, 'a');
      expect(edges.single.toUserId, 'b');
      expect(edges.single.amountCents, 500);
    });

    test('balances always sum to zero in valid input and are fully settled',
        () {
      // a owes 1000, b owes 500, c is owed 1500.
      final edges =
          DebtSimplifier.minimize({'a': -1000, 'b': -500, 'c': 1500});
      final totalPaid = edges.fold<int>(0, (s, e) => s + e.amountCents);
      expect(totalPaid, 1500);
      // Everyone pays the single creditor: at most n-1 = 2 payments.
      expect(edges.length, lessThanOrEqualTo(2));
      for (final e in edges) {
        expect(e.toUserId, 'c');
      }
    });

    test('minimises payment count by matching largest debts first', () {
      // Two debtors, two creditors of equal sizes -> 2 payments, not 4.
      final edges = DebtSimplifier.minimize({
        'a': -1000,
        'b': -1000,
        'c': 1000,
        'd': 1000,
      });
      expect(edges, hasLength(2));
      final total = edges.fold<int>(0, (s, e) => s + e.amountCents);
      expect(total, 2000);
    });

    test('output is deterministic', () {
      final first = DebtSimplifier.minimize({'a': -300, 'b': 100, 'c': 200});
      final second = DebtSimplifier.minimize({'c': 200, 'a': -300, 'b': 100});
      expect(first, second);
    });
  });
}
