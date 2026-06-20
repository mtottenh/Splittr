import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../state/providers.dart';

/// Prompts for a new person's name and creates them via the engine. Returns the
/// new person's user id, or `null` if cancelled.
Future<String?> showAddPersonDialog(BuildContext context, WidgetRef ref) {
  return showDialog<String>(
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

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (!_formKey.currentState!.validate()) return;
    final id =
        await ref.read(appProvider.notifier).addPerson(_name.text.trim());
    if (mounted) Navigator.of(context).pop(id);
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Add a person'),
      content: Form(
        key: _formKey,
        child: TextFormField(
          controller: _name,
          autofocus: true,
          textCapitalization: TextCapitalization.words,
          decoration: const InputDecoration(labelText: 'Name'),
          validator: (v) =>
              (v == null || v.trim().isEmpty) ? 'Enter a name' : null,
          onFieldSubmitted: (_) => _submit(),
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
