import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../domain/models/user.dart';
import '../../state/providers.dart';

/// Prompts for a new person's name (and optional email) and creates them.
/// Returns the created [AppUser], or `null` if cancelled.
Future<AppUser?> showAddPersonDialog(BuildContext context, WidgetRef ref) {
  return showDialog<AppUser>(
    context: context,
    builder: (context) => const _AddPersonDialog(),
  );
}

class _AddPersonDialog extends ConsumerStatefulWidget {
  const _AddPersonDialog();

  @override
  ConsumerState<_AddPersonDialog> createState() => _AddPersonDialogState();
}

class _AddPersonDialogState extends ConsumerState<_AddPersonDialog> {
  final _formKey = GlobalKey<FormState>();
  final _name = TextEditingController();
  final _email = TextEditingController();

  @override
  void dispose() {
    _name.dispose();
    _email.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (!_formKey.currentState!.validate()) return;
    final user = await ref
        .read(appControllerProvider.notifier)
        .addPerson(name: _name.text, email: _email.text);
    if (mounted) Navigator.of(context).pop(user);
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Add a person'),
      content: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextFormField(
              controller: _name,
              autofocus: true,
              textCapitalization: TextCapitalization.words,
              decoration: const InputDecoration(labelText: 'Name'),
              validator: (v) =>
                  (v == null || v.trim().isEmpty) ? 'Enter a name' : null,
              onFieldSubmitted: (_) => _submit(),
            ),
            const SizedBox(height: 12),
            TextFormField(
              controller: _email,
              keyboardType: TextInputType.emailAddress,
              decoration:
                  const InputDecoration(labelText: 'Email (optional)'),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(onPressed: _submit, child: const Text('Add')),
      ],
    );
  }
}
