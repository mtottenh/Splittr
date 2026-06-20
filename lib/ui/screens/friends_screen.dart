import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../state/providers.dart';
import '../widgets/add_person_dialog.dart';
import '../widgets/balance_label.dart';
import '../widgets/empty_state.dart';
import '../widgets/user_avatar.dart';
import 'friend_detail_screen.dart';

/// Lists everyone the user shares expenses with and the balance with each.
class FriendsScreen extends ConsumerWidget {
  const FriendsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final friends = ref.watch(appProvider).requireValue.friends;

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
                return ListTile(
                  leading: UserAvatar(name: friend.name, id: friend.userId),
                  title: Text(friend.name),
                  trailing: BalanceLabel(
                    netCents: friend.netCents,
                    currencyCode: 'USD',
                  ),
                  onTap: () => Navigator.of(context).push(
                    MaterialPageRoute<void>(
                      builder: (_) =>
                          FriendDetailScreen(friendId: friend.userId),
                    ),
                  ),
                );
              },
            ),
    );
  }
}
