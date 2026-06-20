import '../models/balance.dart';
import '../models/expense.dart';
import '../models/settlement.dart';
import 'debt_simplifier.dart';

/// Computes balances from the raw ledger of expenses and settlements.
///
/// Two views are produced:
///  * [netBalances] — each user's overall position (paid minus owed). This is
///    what drives the simplified "settle up" suggestions.
///  * [pairwiseDebts] — the actual "who owes whom" graph, preserved per pair so
///    the app can show un-simplified balances when a group prefers them.
class BalanceCalculator {
  const BalanceCalculator._();

  /// Net position per user, in cents. Positive = is owed; negative = owes.
  /// The returned values always sum to zero.
  static Map<String, int> netBalances({
    required List<Expense> expenses,
    required List<Settlement> settlements,
  }) {
    final net = <String, int>{};
    void add(String user, int delta) =>
        net[user] = (net[user] ?? 0) + delta;

    for (final e in expenses) {
      e.paidBy.forEach((user, paid) => add(user, paid));
      for (final s in e.splits) {
        add(s.userId, -s.owedCents);
      }
    }
    for (final s in settlements) {
      // Paying down a debt: the payer's position rises, the payee's falls.
      add(s.fromUserId, s.amountCents);
      add(s.toUserId, -s.amountCents);
    }

    net.removeWhere((_, value) => value == 0);
    return net;
  }

  /// The per-pair debt graph. Each [DebtEdge] is `from owes to`.
  ///
  /// For single-payer expenses (the overwhelming majority) this is exact: every
  /// participant simply owes the payer their share. For multi-payer expenses it
  /// allocates each participant's share across payers via the same minimal
  /// matching used for simplification, keeping the result exact to the cent.
  static List<DebtEdge> pairwiseDebts({
    required List<Expense> expenses,
    required List<Settlement> settlements,
  }) {
    // pair value = amount the lexicographically-greater id owes the lesser id.
    final pair = <String, int>{};
    String key(String a, String b) => a.compareTo(b) < 0 ? '$a|$b' : '$b|$a';

    void addDebt(String debtor, String creditor, int amount) {
      if (amount == 0 || debtor == creditor) return;
      final k = key(debtor, creditor);
      // If debtor is the lesser id, "max owes min" decreases; else it increases.
      pair[k] = (pair[k] ?? 0) + (debtor.compareTo(creditor) < 0 ? -amount : amount);
    }

    for (final e in expenses) {
      // Within a single expense, match owers to payers minimally.
      final within = <String, int>{};
      for (final s in e.splits) {
        within[s.userId] = (within[s.userId] ?? 0) - s.owedCents;
      }
      e.paidBy.forEach((user, paid) {
        within[user] = (within[user] ?? 0) + paid;
      });
      for (final edge in DebtSimplifier.minimize(within)) {
        addDebt(edge.fromUserId, edge.toUserId, edge.amountCents);
      }
    }

    for (final s in settlements) {
      // A payment cancels an existing debt from payer to payee.
      addDebt(s.toUserId, s.fromUserId, s.amountCents);
    }

    final edges = <DebtEdge>[];
    final keys = pair.keys.toList()..sort();
    for (final k in keys) {
      final value = pair[k]!;
      if (value == 0) continue;
      final parts = k.split('|');
      final lesser = parts[0];
      final greater = parts[1];
      if (value > 0) {
        edges.add(DebtEdge(
            fromUserId: greater, toUserId: lesser, amountCents: value));
      } else {
        edges.add(DebtEdge(
            fromUserId: lesser, toUserId: greater, amountCents: -value));
      }
    }
    return edges;
  }

  /// Suggested payments to settle everyone up. When [simplify] is true the
  /// whole-group netting is used; otherwise the actual pairwise debts are
  /// returned unchanged.
  static List<DebtEdge> settleUpSuggestions({
    required List<Expense> expenses,
    required List<Settlement> settlements,
    required bool simplify,
  }) {
    if (simplify) {
      return DebtSimplifier.minimize(
        netBalances(expenses: expenses, settlements: settlements),
      );
    }
    return pairwiseDebts(expenses: expenses, settlements: settlements);
  }
}
