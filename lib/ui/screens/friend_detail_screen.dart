import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/money.dart';
import '../../core/theme.dart';
import '../../src/rust/dto.dart';
import '../../state/providers.dart';
import '../widgets/balance_label.dart';
import '../widgets/user_avatar.dart';
import 'add_expense_screen.dart';

/// Shows the running balance and shared history with a single friend, across
/// every group the two share, plus non-group expenses (#31).
class FriendDetailScreen extends ConsumerWidget {
  const FriendDetailScreen({super.key, required this.friendId});
  final String friendId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final detail = ref.watch(friendDetailProvider(friendId));
    final name = detail.valueOrNull?.name ?? 'Friend';

    return Scaffold(
      appBar: AppBar(
        title: Text(name),
        actions: [
          if ((detail.valueOrNull?.netCents ?? 0) != 0)
            TextButton(
              onPressed: () => _settleUp(context, ref, detail.value!),
              child: const Text('Settle up'),
            ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => Navigator.of(context).push(
          MaterialPageRoute<void>(
            builder: (_) =>
                AddExpenseScreen(friendId: friendId, friendName: name),
            fullscreenDialog: true,
          ),
        ),
        icon: const Icon(Icons.add),
        label: const Text('Add expense'),
      ),
      body: detail.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('Error: $e')),
        data: (friend) {
          if (friend == null) {
            return const Center(child: Text('Friend not found'));
          }
          return _Body(friend: friend);
        },
      ),
    );
  }

  /// Record a payment that clears the friend balance (positive = they owe you,
  /// so they pay you; negative = you pay them).
  Future<void> _settleUp(
      BuildContext context, WidgetRef ref, FriendDetailDto friend) async {
    final me = ref.read(appProvider).requireValue.myUserId;
    final net = friend.netCents;
    final from = net > 0 ? friend.userId : me;
    final to = net > 0 ? me : friend.userId;
    await ref.read(appProvider.notifier).recordNonGroupSettlement(
          from: from,
          to: to,
          amountCents: net.abs(),
        );
  }
}

class _Body extends ConsumerWidget {
  const _Body({required this.friend});
  final FriendDetailDto friend;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final net = friend.netCents;
    return ListView(
      padding: const EdgeInsets.only(bottom: 96),
      children: [
        Padding(
          padding: const EdgeInsets.all(16),
          child: Row(
            children: [
              UserAvatar(name: friend.name, id: friend.userId, radius: 28),
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
        if (friend.shared.isEmpty)
          const Padding(
            padding: EdgeInsets.all(32),
            child: Center(child: Text('No shared expenses yet')),
          )
        else
          for (final expense in friend.shared)
            ListTile(
              leading: const CircleAvatar(
                  child: Icon(Icons.receipt_long, size: 20)),
              title: Text(expense.description),
              subtitle: Text(
                '${expense.groupName ?? "Non-group"} · '
                '${Money.format(expense.totalCents, code: expense.currency)}',
              ),
              trailing: BalanceLabel(
                netCents: expense.myNetCents,
                currencyCode: expense.currency,
                youArePositive: 'you lent',
                youAreNegative: 'you borrowed',
                settledText: '—',
              ),
              onTap: () => Navigator.of(context).push(
                MaterialPageRoute<void>(
                  builder: (_) => AddExpenseScreen(existing: expense),
                  fullscreenDialog: true,
                ),
              ),
            ),
      ],
    );
  }
}
