import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/money.dart';
import '../../core/theme.dart';
import '../../src/rust/dto.dart';
import '../../state/providers.dart';
import '../widgets/balance_label.dart';
import '../widgets/user_avatar.dart';

/// Shows the running balance and shared history with a single friend, across
/// every group the two share.
class FriendDetailScreen extends ConsumerWidget {
  const FriendDetailScreen({super.key, required this.friendId});
  final String friendId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final detail = ref.watch(friendDetailProvider(friendId));

    return Scaffold(
      appBar: AppBar(title: Text(detail.valueOrNull?.name ?? 'Friend')),
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
}

class _Body extends StatelessWidget {
  const _Body({required this.friend});
  final FriendDetailDto friend;

  @override
  Widget build(BuildContext context) {
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
                '${expense.groupName} · ${Money.format(expense.totalCents)}',
              ),
              trailing: BalanceLabel(
                netCents: expense.myNetCents,
                currencyCode: 'USD',
                youArePositive: 'you lent',
                youAreNegative: 'you borrowed',
                settledText: '—',
              ),
            ),
      ],
    );
  }
}
