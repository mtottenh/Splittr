import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../state/providers.dart';
import '../widgets/user_avatar.dart';

/// Lets the user edit their own profile and see app information.
class AccountScreen extends ConsumerWidget {
  const AccountScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final data = ref.watch(appProvider).requireValue;

    return Scaffold(
      appBar: AppBar(title: const Text('Account')),
      body: ListView(
        children: [
          const SizedBox(height: 16),
          Center(
            child: UserAvatar(name: data.myName, id: data.myUserId, radius: 40),
          ),
          const SizedBox(height: 12),
          Center(
            child:
                Text(data.myName, style: Theme.of(context).textTheme.titleLarge),
          ),
          const SizedBox(height: 8),
          Center(
            child: TextButton.icon(
              onPressed: () => _editProfile(context, ref, data.myName),
              icon: const Icon(Icons.edit),
              label: const Text('Edit profile'),
            ),
          ),
          const Divider(height: 32),
          ListTile(
            leading: const Icon(Icons.groups),
            title: const Text('Groups'),
            trailing: Text('${data.groups.length}'),
          ),
          ListTile(
            leading: const Icon(Icons.people),
            title: const Text('Friends'),
            trailing: Text('${data.friends.length}'),
          ),
          const Divider(height: 32),
          const AboutListTile(
            applicationName: 'Splittr',
            applicationVersion: '1.0.0',
            icon: Icon(Icons.call_split),
            applicationLegalese:
                'A cross-platform expense sharing app: a Rust engine with a '
                'Flutter shell.',
            child: Text('About'),
          ),
        ],
      ),
    );
  }

  Future<void> _editProfile(
      BuildContext context, WidgetRef ref, String currentName) async {
    final controller = TextEditingController(text: currentName);
    final name = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Edit profile'),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: const InputDecoration(labelText: 'Name'),
          textCapitalization: TextCapitalization.words,
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(controller.text.trim()),
            child: const Text('Save'),
          ),
        ],
      ),
    );
    controller.dispose();
    if (name != null && name.isNotEmpty) {
      await ref.read(appProvider.notifier).setMyName(name);
    }
  }
}
