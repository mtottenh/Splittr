import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../state/engine.dart';

/// First-run identity setup (#34/ADR-0005): create a new local identity (showing
/// its recovery phrase once) or restore one from a recovery phrase. On success
/// it invalidates [identityEstablishedProvider] so the app boots into the shell.
class OnboardingScreen extends ConsumerWidget {
  const OnboardingScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Scaffold(
      body: SafeArea(
        child: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 420),
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Icon(Icons.call_split,
                      size: 56, color: Theme.of(context).colorScheme.primary),
                  const SizedBox(height: 16),
                  Text('Welcome to Splittr',
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.headlineSmall),
                  const SizedBox(height: 8),
                  const Text(
                    'Your data lives on your device, signed by an identity only '
                    'you hold. Create one, or restore an existing identity.',
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 32),
                  FilledButton.icon(
                    onPressed: () => _createNew(context, ref),
                    icon: const Icon(Icons.add),
                    label: const Text('Create a new identity'),
                  ),
                  const SizedBox(height: 12),
                  OutlinedButton.icon(
                    onPressed: () => _restore(context, ref),
                    icon: const Icon(Icons.vpn_key_outlined),
                    label: const Text('Restore from recovery phrase'),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _createNew(BuildContext context, WidgetRef ref) async {
    final bootstrap = await ref.read(identityBootstrapProvider.future);
    final phrase = await bootstrap.createNew();
    if (!context.mounted) return;
    final confirmed = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (context) => _PhraseConfirmDialog(phrase: phrase),
    );
    if (confirmed ?? false) {
      ref.invalidate(identityEstablishedProvider);
    }
  }

  Future<void> _restore(BuildContext context, WidgetRef ref) async {
    final phrase = await showDialog<String>(
      context: context,
      builder: (context) => const _RestoreDialog(),
    );
    if (phrase == null || !context.mounted) return;

    final bootstrap = await ref.read(identityBootstrapProvider.future);
    final restored = await bootstrap.restore(phrase);
    if (!context.mounted) return;
    if (restored) {
      ref.invalidate(identityEstablishedProvider);
    } else {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('That recovery phrase is not valid.')),
      );
    }
  }
}

/// Shows the new identity's recovery phrase once, gated by a "written it down"
/// confirmation so the user can't skip backing it up.
class _PhraseConfirmDialog extends StatefulWidget {
  const _PhraseConfirmDialog({required this.phrase});
  final String phrase;

  @override
  State<_PhraseConfirmDialog> createState() => _PhraseConfirmDialogState();
}

class _PhraseConfirmDialogState extends State<_PhraseConfirmDialog> {
  bool _acknowledged = false;

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Your recovery phrase'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Write these 24 words down and keep them safe. They are the only way '
            'to restore your identity on a new device; nobody can recover them '
            'for you.',
          ),
          const SizedBox(height: 16),
          SelectableText(
            widget.phrase,
            style: const TextStyle(fontFamily: 'monospace', height: 1.5),
          ),
          const SizedBox(height: 8),
          CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            value: _acknowledged,
            onChanged: (v) => setState(() => _acknowledged = v ?? false),
            title: const Text("I've written it down"),
          ),
        ],
      ),
      actions: [
        FilledButton(
          onPressed:
              _acknowledged ? () => Navigator.of(context).pop(true) : null,
          child: const Text('Continue'),
        ),
      ],
    );
  }
}

/// Owns its own controller (disposed after the dialog closes) and pops the
/// entered phrase, or null on cancel.
class _RestoreDialog extends StatefulWidget {
  const _RestoreDialog();

  @override
  State<_RestoreDialog> createState() => _RestoreDialogState();
}

class _RestoreDialogState extends State<_RestoreDialog> {
  final _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Restore from recovery phrase'),
      content: TextField(
        controller: _controller,
        autofocus: true,
        minLines: 2,
        maxLines: 4,
        decoration: const InputDecoration(
          labelText: 'Recovery phrase',
          hintText: 'Enter your 24 words separated by spaces',
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(_controller.text),
          child: const Text('Restore'),
        ),
      ],
    );
  }
}
