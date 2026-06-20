import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../domain/models/balance.dart';
import '../../state/providers.dart';
import '../widgets/add_person_dialog.dart';
import '../widgets/balance_label.dart';
import '../widgets/empty_state.dart';
import '../widgets/user_avatar.dart';
import 'friend_detail_screen.dart';

/// Net amount owed between the current user and [friendId], from the current
/// user's perspective (positive = the friend owes you).
int friendNet(List<DebtEdge> edges, String me, String friendId) {
  var net = 0;
  for (final e in edges) {
    if (e.fromUserId == friendId && e.toUserId == me) net += e.amountCents;
    if (e.fromUserId == me && e.toUserId == friendId) net -= e.amountCents;
  }
  return net;
}

/// Lists everyone the user shares expenses with and the balance with each.
class FriendsScreen extends ConsumerWidget {
  const FriendsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(appControllerProvider);
    final edges = ref.watch(overallPairwiseProvider);
    final friends =
        state.users.where((u) => u.id != state.currentUserId).toList();

    return Scaffold(
      appBar: AppBar(
        title: const Text('Friends'),
        actions: [
          IconButton(
            tooltip: 'Add friend',
            icon: const Icon(Icons.person_add),
            onPressed: () => showAddPersonDialog(context, ref),
          ),
        ],
      ),
      body: friends.isEmpty
          ? EmptyState(
              icon: Icons.person,
              title: 'No friends yet',
              message: 'Add people to start splitting expenses with them.',
              action: FilledButton.icon(
                onPressed: () => showAddPersonDialog(context, ref),
                icon: const Icon(Icons.person_add),
                label: const Text('Add a friend'),
              ),
            )
          : ListView.separated(
              padding: const EdgeInsets.only(bottom: 96),
              itemCount: friends.length,
              separatorBuilder: (_, _) => const Divider(height: 1),
              itemBuilder: (context, i) {
                final friend = friends[i];
                final net = friendNet(edges, state.currentUserId, friend.id);
                return ListTile(
                  leading: UserAvatar(user: friend),
                  title: Text(friend.name),
                  subtitle: friend.email == null ? null : Text(friend.email!),
                  trailing: BalanceLabel(netCents: net, currencyCode: 'USD'),
                  onTap: () => Navigator.of(context).push(
                    MaterialPageRoute<void>(
                      builder: (_) =>
                          FriendDetailScreen(friendId: friend.id),
                    ),
                  ),
                );
              },
            ),
    );
  }
}
