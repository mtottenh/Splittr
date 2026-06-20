import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/money.dart';
import '../../core/theme.dart';
import '../../domain/models/expense.dart';
import '../../state/providers.dart';
import '../widgets/balance_label.dart';
import '../widgets/user_avatar.dart';
import 'add_expense_screen.dart';
import 'friends_screen.dart';

/// Shows the running balance and shared history with a single friend, across
/// every group and non-group expense the two share.
class FriendDetailScreen extends ConsumerWidget {
  const FriendDetailScreen({super.key, required this.friendId});
  final String friendId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(appControllerProvider);
    final friend = state.userById(friendId);
    if (friend == null) {
      return const Scaffold(body: Center(child: Text('Friend not found')));
    }
    final me = state.currentUserId;
    final net = friendNet(ref.watch(overallPairwiseProvider), me, friendId);

    final shared = state.expenses
        .where((e) =>
            e.participantIds.contains(me) &&
            e.participantIds.contains(friendId))
        .toList()
      ..sort((a, b) => b.date.compareTo(a.date));

    return Scaffold(
      appBar: AppBar(title: Text(friend.name)),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => Navigator.of(context).push(
          MaterialPageRoute<void>(
            builder: (_) => AddExpenseScreen(friendId: friendId),
            fullscreenDialog: true,
          ),
        ),
        icon: const Icon(Icons.add),
        label: const Text('Add expense'),
      ),
      body: ListView(
        padding: const EdgeInsets.only(bottom: 96),
        children: [
          Padding(
            padding: const EdgeInsets.all(16),
            child: Row(
              children: [
                UserAvatar(user: friend, radius: 28),
                const SizedBox(width: 16),
                Expanded(
                  child: Text(
                    net == 0
                        ? 'You are settled up'
                        : net > 0
                            ? '${friend.name} owes you ${Money.formatAbs(net)}'
                            : 'You owe ${friend.name} ${Money.formatAbs(net)}',
                    style: Theme.of(context).textTheme.titleMedium?.copyWith(
                          color: AppTheme.balanceColor(
                              net, Theme.of(context).colorScheme),
                          fontWeight: FontWeight.w600,
                        ),
                  ),
                ),
              ],
            ),
          ),
          const Divider(height: 1),
          if (shared.isEmpty)
            const Padding(
              padding: EdgeInsets.all(32),
              child: Center(child: Text('No shared expenses yet')),
            )
          else
            for (final expense in shared)
              _SharedExpenseTile(expense: expense, friendId: friendId),
        ],
      ),
    );
  }
}

class _SharedExpenseTile extends ConsumerWidget {
  const _SharedExpenseTile({required this.expense, required this.friendId});
  final Expense expense;
  final String friendId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(appControllerProvider);
    final me = state.currentUserId;
    final myShare = expense.paidByUser(me) - expense.owedByUser(me);
    final groupName =
        expense.groupId == null ? 'Non-group' : state.groupById(expense.groupId!)?.name ?? '';

    return ListTile(
      leading: const CircleAvatar(child: Icon(Icons.receipt_long, size: 20)),
      title: Text(expense.description),
      subtitle: Text(
        '$groupName · ${Money.format(expense.totalCents, code: expense.currencyCode)}',
      ),
      trailing: BalanceLabel(
        netCents: myShare,
        currencyCode: expense.currencyCode,
        youArePositive: 'you lent',
        youAreNegative: 'you borrowed',
        settledText: '—',
      ),
      onTap: () => Navigator.of(context).push(
        MaterialPageRoute<void>(
          builder: (_) => AddExpenseScreen(
            groupId: expense.groupId,
            existing: expense,
          ),
          fullscreenDialog: true,
        ),
      ),
    );
  }
}
