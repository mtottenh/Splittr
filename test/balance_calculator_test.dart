import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/domain/models/enums.dart';
import 'package:splittr/domain/models/expense.dart';
import 'package:splittr/domain/models/settlement.dart';
import 'package:splittr/domain/services/balance_calculator.dart';
import 'package:splittr/domain/services/split_calculator.dart';

Expense expense({
  required String id,
  required int totalCents,
  required String payer,
  required List<String> participants,
}) {
  return Expense(
    id: id,
    groupId: 'g1',
    description: 'e$id',
    totalCents: totalCents,
    currencyCode: 'USD',
    paidBy: {payer: totalCents},
    splits: SplitCalculator.equal(totalCents: totalCents, userIds: participants),
    splitType: SplitType.equal,
    categoryId: 'general',
    date: DateTime(2024, 1, 1),
    createdAt: DateTime(2024, 1, 1),
  );
}

void main() {
  group('netBalances', () {
    test('payer is owed their outlay minus their own share', () {
      // a pays 30 for a, b, c -> a is owed 20, b and c owe 10 each.
      final net = BalanceCalculator.netBalances(
        expenses: [
          expense(id: '1', totalCents: 3000, payer: 'a', participants: [
            'a',
            'b',
            'c',
          ]),
        ],
        settlements: [],
      );
      expect(net['a'], 2000);
      expect(net['b'], -1000);
      expect(net['c'], -1000);
    });

    test('always sums to zero', () {
      final net = BalanceCalculator.netBalances(
        expenses: [
          expense(id: '1', totalCents: 1000, payer: 'a', participants: ['a', 'b']),
          expense(id: '2', totalCents: 700, payer: 'b', participants: ['a', 'b', 'c']),
        ],
        settlements: [],
      );
      expect(net.values.fold(0, (s, v) => s + v), 0);
    });

    test('settlements reduce balances', () {
      final net = BalanceCalculator.netBalances(
        expenses: [
          expense(id: '1', totalCents: 1000, payer: 'a', participants: ['a', 'b']),
        ],
        settlements: [
          Settlement(
            id: 's1',
            groupId: 'g1',
            fromUserId: 'b',
            toUserId: 'a',
            amountCents: 500,
            currencyCode: 'USD',
            date: DateTime(2024, 1, 2),
          ),
        ],
      );
      // b owed 5.00, paid it back -> everyone settled.
      expect(net, isEmpty);
    });
  });

  group('pairwiseDebts', () {
    test('single payer: each participant owes the payer', () {
      final edges = BalanceCalculator.pairwiseDebts(
        expenses: [
          expense(id: '1', totalCents: 3000, payer: 'a', participants: ['a', 'b', 'c']),
        ],
        settlements: [],
      );
      expect(edges, hasLength(2));
      for (final e in edges) {
        expect(e.toUserId, 'a');
        expect(e.amountCents, 1000);
      }
    });

    test('settlement clears the matching pairwise debt', () {
      final edges = BalanceCalculator.pairwiseDebts(
        expenses: [
          expense(id: '1', totalCents: 2000, payer: 'a', participants: ['a', 'b']),
        ],
        settlements: [
          Settlement(
            id: 's1',
            groupId: 'g1',
            fromUserId: 'b',
            toUserId: 'a',
            amountCents: 1000,
            currencyCode: 'USD',
            date: DateTime(2024, 1, 2),
          ),
        ],
      );
      expect(edges, isEmpty);
    });
  });

  group('settleUpSuggestions', () {
    // a owes b 5.00 (expense 1); b owes c 5.00 (expense 2).
    final chain = [
      expense(id: '1', totalCents: 1000, payer: 'b', participants: ['a', 'b']),
      expense(id: '2', totalCents: 1000, payer: 'c', participants: ['b', 'c']),
    ];

    test('simplify collapses the chain so a pays c directly', () {
      final suggestions = BalanceCalculator.settleUpSuggestions(
        expenses: chain,
        settlements: const [],
        simplify: true,
      );
      expect(suggestions, hasLength(1));
      expect(suggestions.single.fromUserId, 'a');
      expect(suggestions.single.toUserId, 'c');
      expect(suggestions.single.amountCents, 500);
    });

    test('without simplify the original pairwise debts are kept', () {
      final suggestions = BalanceCalculator.settleUpSuggestions(
        expenses: chain,
        settlements: const [],
        simplify: false,
      );
      // Two separate debts: a->b and b->c.
      expect(suggestions, hasLength(2));
      final total = suggestions.fold<int>(0, (s, e) => s + e.amountCents);
      expect(total, 1000);
    });
  });
}
