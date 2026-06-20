import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/root_vault.dart';

/// Obtains the identity (root) seed for a privileged action (reveal recovery
/// phrase, enrol/revoke a device). When app lock is on the root is PIN-sealed
/// (#34), so this prompts for the PIN and unseals it; otherwise it returns the
/// stored seed directly. Returns null if the user cancels.
Future<List<int>?> obtainRootSeed(BuildContext context, WidgetRef ref) async {
  final vault = await ref.read(rootVaultProvider.future);
  if (!await vault.isSealed()) {
    return vault.loadSeed();
  }
  while (true) {
    if (!context.mounted) return null;
    final pin = await promptPin(context);
    if (pin == null) return null; // cancelled
    final seed = await vault.loadSeed(pin: pin);
    if (seed != null) return seed;
    if (!context.mounted) return null;
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(content: Text('Incorrect PIN')),
    );
  }
}

/// Prompt for the app-lock PIN, returning it (unverified) or null on cancel.
Future<String?> promptPin(BuildContext context) => showDialog<String>(
      context: context,
      builder: (context) => const _PinDialog(),
    );

class _PinDialog extends StatefulWidget {
  const _PinDialog();

  @override
  State<_PinDialog> createState() => _PinDialogState();
}

class _PinDialogState extends State<_PinDialog> {
  final _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    void submit() => Navigator.of(context).pop(_controller.text);
    return AlertDialog(
      title: const Text('Enter your PIN'),
      content: TextField(
        controller: _controller,
        autofocus: true,
        obscureText: true,
        keyboardType: TextInputType.number,
        inputFormatters: [FilteringTextInputFormatter.digitsOnly],
        decoration: const InputDecoration(labelText: 'PIN'),
        onSubmitted: (_) => submit(),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(onPressed: submit, child: const Text('Unlock')),
      ],
    );
  }
}
