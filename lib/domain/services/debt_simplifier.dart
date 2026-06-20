import '../models/balance.dart';

/// Reduces a set of net balances to a small set of payments that settle the
/// whole group ("simplify debts" in Splitwise terms).
///
/// Uses a greedy largest-creditor / largest-debtor match. This is the classic
/// minimal-cash-flow heuristic: each payment zeroes out at least one party, so
/// the number of payments never exceeds `n - 1` for `n` non-zero members, which
/// is optimal for the common cases the app produces. Output is deterministic
/// (ties broken by user id) so the UI is stable between rebuilds.
class DebtSimplifier {
  const DebtSimplifier._();

  static List<DebtEdge> minimize(Map<String, int> netBalances) {
    final creditors = <_Holder>[];
    final debtors = <_Holder>[];

    for (final entry in netBalances.entries) {
      if (entry.value > 0) {
        creditors.add(_Holder(entry.key, entry.value));
      } else if (entry.value < 0) {
        debtors.add(_Holder(entry.key, -entry.value));
      }
    }

    int byAmountDescThenId(_Holder a, _Holder b) {
      final cmp = b.amount.compareTo(a.amount);
      return cmp != 0 ? cmp : a.id.compareTo(b.id);
    }

    creditors.sort(byAmountDescThenId);
    debtors.sort(byAmountDescThenId);

    final edges = <DebtEdge>[];
    var i = 0;
    var j = 0;
    while (i < debtors.length && j < creditors.length) {
      final debtor = debtors[i];
      final creditor = creditors[j];
      final pay = debtor.amount < creditor.amount
          ? debtor.amount
          : creditor.amount;

      if (pay > 0) {
        edges.add(DebtEdge(
          fromUserId: debtor.id,
          toUserId: creditor.id,
          amountCents: pay,
        ));
      }

      debtor.amount -= pay;
      creditor.amount -= pay;
      if (debtor.amount == 0) i++;
      if (creditor.amount == 0) j++;
    }

    return edges;
  }
}

class _Holder {
  _Holder(this.id, this.amount);
  final String id;
  int amount;
}
