import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../state/providers.dart';
import '../widgets/add_person_dialog.dart';
import '../widgets/user_avatar.dart';
import 'group_detail_screen.dart';

/// Form for creating a new group: name, emoji, currency, members and whether to
/// simplify debts.
class CreateGroupScreen extends ConsumerStatefulWidget {
  const CreateGroupScreen({super.key});

  @override
  ConsumerState<CreateGroupScreen> createState() => _CreateGroupScreenState();
}

class _CreateGroupScreenState extends ConsumerState<CreateGroupScreen> {
  final _formKey = GlobalKey<FormState>();
  final _name = TextEditingController();
  final _selectedMemberIds = <String>{};

  String _emoji = '🧾';
  String _currency = 'USD';
  bool _simplify = true;

  static const _emojis = ['🧾', '🏠', '✈️', '🍽️', '🎉', '🚗', '🏖️', '⚽'];
  static const _currencies = ['USD', 'EUR', 'GBP', 'CAD', 'AUD', 'INR', 'JPY'];

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  Future<void> _create() async {
    if (!_formKey.currentState!.validate()) return;
    final group = await ref.read(appControllerProvider.notifier).createGroup(
          name: _name.text,
          memberIds: _selectedMemberIds.toList(),
          currencyCode: _currency,
          emoji: _emoji,
          simplifyDebts: _simplify,
        );
    if (!mounted) return;
    await Navigator.of(context).pushReplacement(
      MaterialPageRoute<void>(
        builder: (_) => GroupDetailScreen(groupId: group.id),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(appControllerProvider);
    final others =
        state.users.where((u) => u.id != state.currentUserId).toList();

    return Scaffold(
      appBar: AppBar(title: const Text('Create group')),
      body: Form(
        key: _formKey,
        child: ListView(
          padding: const EdgeInsets.all(16),
          children: [
            Row(
              children: [
                _EmojiPicker(
                  value: _emoji,
                  options: _emojis,
                  onChanged: (e) => setState(() => _emoji = e),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: TextFormField(
                    controller: _name,
                    textCapitalization: TextCapitalization.words,
                    decoration: const InputDecoration(
                      labelText: 'Group name',
                      hintText: 'e.g. Iceland trip',
                    ),
                    validator: (v) => (v == null || v.trim().isEmpty)
                        ? 'Enter a group name'
                        : null,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),
            DropdownButtonFormField<String>(
              initialValue: _currency,
              decoration: const InputDecoration(labelText: 'Currency'),
              items: [
                for (final c in _currencies)
                  DropdownMenuItem(value: c, child: Text(c)),
              ],
              onChanged: (v) => setState(() => _currency = v ?? 'USD'),
            ),
            const SizedBox(height: 16),
            SwitchListTile(
              value: _simplify,
              onChanged: (v) => setState(() => _simplify = v),
              title: const Text('Simplify debts'),
              subtitle: const Text(
                  'Reduce the number of payments needed to settle up'),
              contentPadding: EdgeInsets.zero,
            ),
            const Divider(height: 32),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text('Members', style: Theme.of(context).textTheme.titleMedium),
                TextButton.icon(
                  onPressed: () async {
                    final user = await showAddPersonDialog(context, ref);
                    if (user != null) {
                      setState(() => _selectedMemberIds.add(user.id));
                    }
                  },
                  icon: const Icon(Icons.person_add),
                  label: const Text('New'),
                ),
              ],
            ),
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: UserAvatar(user: state.currentUser),
              title: Text('${state.currentUser.name} (you)'),
              trailing: const Icon(Icons.check_circle, color: Colors.green),
            ),
            for (final user in others)
              CheckboxListTile(
                contentPadding: EdgeInsets.zero,
                value: _selectedMemberIds.contains(user.id),
                onChanged: (checked) => setState(() {
                  if (checked ?? false) {
                    _selectedMemberIds.add(user.id);
                  } else {
                    _selectedMemberIds.remove(user.id);
                  }
                }),
                secondary: UserAvatar(user: user),
                title: Text(user.name),
                subtitle: user.email == null ? null : Text(user.email!),
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

class _EmojiPicker extends StatelessWidget {
  const _EmojiPicker({
    required this.value,
    required this.options,
    required this.onChanged,
  });

  final String value;
  final List<String> options;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      borderRadius: BorderRadius.circular(12),
      onTap: () async {
        final picked = await showModalBottomSheet<String>(
          context: context,
          builder: (context) => GridView.count(
            crossAxisCount: 4,
            padding: const EdgeInsets.all(16),
            shrinkWrap: true,
            children: [
              for (final e in options)
                IconButton(
                  onPressed: () => Navigator.of(context).pop(e),
                  icon: Text(e, style: const TextStyle(fontSize: 28)),
                ),
            ],
          ),
        );
        if (picked != null) onChanged(picked);
      },
      child: Container(
        width: 56,
        height: 56,
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.primaryContainer,
          borderRadius: BorderRadius.circular(12),
        ),
        alignment: Alignment.center,
        child: Text(value, style: const TextStyle(fontSize: 26)),
      ),
    );
  }
}
