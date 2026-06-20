import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../state/providers.dart';
import '../widgets/add_person_dialog.dart';
import '../widgets/user_avatar.dart';
import 'group_detail_screen.dart';

/// Form for creating a new group: a name and its members.
class CreateGroupScreen extends ConsumerStatefulWidget {
  const CreateGroupScreen({super.key});

  @override
  ConsumerState<CreateGroupScreen> createState() => _CreateGroupScreenState();
}

class _CreateGroupScreenState extends ConsumerState<CreateGroupScreen> {
  final _formKey = GlobalKey<FormState>();
  final _name = TextEditingController();
  final _selectedMemberIds = <String>{};

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  Future<void> _create() async {
    if (!_formKey.currentState!.validate()) return;
    final id = await ref
        .read(appProvider.notifier)
        .createGroup(_name.text.trim(), _selectedMemberIds.toList());
    if (!mounted) return;
    await Navigator.of(context).pushReplacement(
      MaterialPageRoute<void>(
        builder: (_) => GroupDetailScreen(groupId: id),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final data = ref.watch(appProvider).requireValue;

    return Scaffold(
      appBar: AppBar(title: const Text('Create group')),
      body: Form(
        key: _formKey,
        child: ListView(
          padding: const EdgeInsets.all(16),
          children: [
            TextFormField(
              controller: _name,
              textCapitalization: TextCapitalization.words,
              decoration: const InputDecoration(
                labelText: 'Group name',
                hintText: 'e.g. Iceland trip',
              ),
              validator: (v) =>
                  (v == null || v.trim().isEmpty) ? 'Enter a group name' : null,
            ),
            const Divider(height: 32),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text('Members', style: Theme.of(context).textTheme.titleMedium),
                TextButton.icon(
                  onPressed: () async {
                    final id = await showAddPersonDialog(context, ref);
                    if (id != null) {
                      setState(() => _selectedMemberIds.add(id));
                    }
                  },
                  icon: const Icon(Icons.person_add),
                  label: const Text('New'),
                ),
              ],
            ),
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: UserAvatar(name: data.myName, id: data.myUserId),
              title: Text('${data.myName} (you)'),
              trailing: const Icon(Icons.check_circle, color: Colors.green),
            ),
            for (final friend in data.friends)
              CheckboxListTile(
                contentPadding: EdgeInsets.zero,
                value: _selectedMemberIds.contains(friend.userId),
                onChanged: (checked) => setState(() {
                  if (checked ?? false) {
                    _selectedMemberIds.add(friend.userId);
                  } else {
                    _selectedMemberIds.remove(friend.userId);
                  }
                }),
                secondary: UserAvatar(name: friend.name, id: friend.userId),
                title: Text(friend.name),
              ),
            const SizedBox(height: 24),
            FilledButton.icon(
              onPressed: _create,
              icon: const Icon(Icons.check),
              label: const Text('Create group'),
            ),
          ],
        ),
      ),
    );
  }
}
