import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../state/providers.dart';
import '../widgets/user_avatar.dart';

/// Lets the user edit their own profile and see app information.
class AccountScreen extends ConsumerWidget {
  const AccountScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(appControllerProvider);
    final me = state.currentUser;
    final friendCount =
        state.users.where((u) => u.id != state.currentUserId).length;

    return Scaffold(
      appBar: AppBar(title: const Text('Account')),
      body: ListView(
        children: [
          const SizedBox(height: 16),
          Center(child: UserAvatar(user: me, radius: 40)),
          const SizedBox(height: 12),
          Center(
            child: Text(me.name,
                style: Theme.of(context).textTheme.titleLarge),
          ),
          if (me.email != null)
            Center(
              child: Text(me.email!,
                  style: Theme.of(context).textTheme.bodyMedium),
            ),
          const SizedBox(height: 8),
          Center(
            child: TextButton.icon(
              onPressed: () => _editProfile(context, ref),
              icon: const Icon(Icons.edit),
              label: const Text('Edit profile'),
            ),
          ),
          const Divider(height: 32),
          ListTile(
            leading: const Icon(Icons.groups),
            title: const Text('Groups'),
            trailing: Text('${state.groups.length}'),
          ),
          ListTile(
            leading: const Icon(Icons.people),
            title: const Text('Friends'),
            trailing: Text('$friendCount'),
          ),
          ListTile(
            leading: const Icon(Icons.receipt_long),
            title: const Text('Expenses'),
            trailing: Text('${state.expenses.length}'),
          ),
          const Divider(height: 32),
          const AboutListTile(
            applicationName: 'Splittr',
            applicationVersion: '1.0.0',
            icon: Icon(Icons.call_split),
            applicationLegalese:
                'A cross-platform expense sharing app built with Flutter.',
            child: Text('About'),
          ),
        ],
      ),
    );
  }

  Future<void> _editProfile(BuildContext context, WidgetRef ref) async {
    final me = ref.read(appControllerProvider).currentUser;
    final nameController = TextEditingController(text: me.name);
    final emailController = TextEditingController(text: me.email ?? '');

    final saved = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Edit profile'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: nameController,
              decoration: const InputDecoration(labelText: 'Name'),
              textCapitalization: TextCapitalization.words,
            ),
            const SizedBox(height: 12),
            TextField(
              controller: emailController,
              decoration: const InputDecoration(labelText: 'Email'),
              keyboardType: TextInputType.emailAddress,
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Save'),
          ),
        ],
      ),
    );

    if (saved ?? false) {
      await ref.read(appControllerProvider.notifier).updateCurrentUser(
            name: nameController.text.trim().isEmpty
                ? 'You'
                : nameController.text.trim(),
            email: emailController.text.trim().isEmpty
                ? null
                : emailController.text.trim(),
          );
    }
    nameController.dispose();
    emailController.dispose();
  }
}
