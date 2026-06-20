import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../state/app_lock.dart';
import '../../state/providers.dart';
import '../../state/root_vault.dart';
import '../root_access.dart';
import '../widgets/user_avatar.dart';
import 'devices_screen.dart';

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
          _AppLockTile(),
          ListTile(
            leading: const Icon(Icons.devices),
            title: const Text('Linked devices'),
            onTap: () => Navigator.of(context).push(
              MaterialPageRoute<void>(builder: (_) => const DevicesScreen()),
            ),
          ),
          ListTile(
            leading: const Icon(Icons.vpn_key_outlined),
            title: const Text('Recovery phrase'),
            subtitle: const Text('Back up your identity'),
            onTap: () => _showRecoveryPhrase(context, ref),
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

  Future<void> _showRecoveryPhrase(BuildContext context, WidgetRef ref) async {
    final seed = await obtainRootSeed(context, ref);
    if (seed == null || !context.mounted) return;
    final phrase = await ref.read(appProvider.notifier).recoveryPhraseFor(seed);
    if (!context.mounted) return;
    await showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Recovery phrase'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              'Write these 24 words down and keep them safe. Anyone with them '
              'can restore your identity; lose them and a lost device cannot be '
              'recovered.',
            ),
            const SizedBox(height: 16),
            SelectableText(
              phrase,
              style: const TextStyle(fontFamily: 'monospace', height: 1.5),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Done'),
          ),
        ],
      ),
    );
  }
}

/// App-lock settings: set / change / remove the PIN (#22). Turning the lock on
/// also seals the identity (root) seed under the PIN; turning it off (or
/// changing it) re-keys that vault, so the lock and the root stay in sync (#34).
class _AppLockTile extends ConsumerWidget {
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final lock = ref.watch(appLockProvider);
    return Column(
      children: [
        ListTile(
          leading: const Icon(Icons.lock_outline),
          title: const Text('App lock'),
          subtitle: Text(lock.pinSet
              ? 'PIN enabled · recovery phrase & device changes need it'
              : 'Off'),
          trailing: TextButton(
            onPressed: () => _setOrChange(context, ref, isChange: lock.pinSet),
            child: Text(lock.pinSet ? 'Change' : 'Set up'),
          ),
        ),
        if (lock.pinSet)
          ListTile(
            leading: const Icon(Icons.lock_open_outlined),
            title: const Text('Remove app lock'),
            onTap: () => _remove(context, ref),
          ),
      ],
    );
  }

  Future<void> _setOrChange(
    BuildContext context,
    WidgetRef ref, {
    required bool isChange,
  }) async {
    final result = await showDialog<({String? oldPin, String newPin})>(
      context: context,
      builder: (context) => _PinSetupDialog(isChange: isChange),
    );
    if (result == null || !context.mounted) return;

    final vault = await ref.read(rootVaultProvider.future);
    if (isChange) {
      // Re-key the vault under the new PIN; a wrong current PIN must not proceed.
      final ok =
          await vault.reseal(oldPin: result.oldPin!, newPin: result.newPin);
      if (!ok) {
        if (context.mounted) _toast(context, 'Current PIN is incorrect');
        return;
      }
      await ref.read(appLockProvider.notifier).setPin(result.newPin);
    } else {
      await ref.read(appLockProvider.notifier).setPin(result.newPin);
      await vault.seal(result.newPin);
    }
  }

  Future<void> _remove(BuildContext context, WidgetRef ref) async {
    final pin = await promptPin(context);
    if (pin == null || !context.mounted) return;
    final vault = await ref.read(rootVaultProvider.future);
    if (!await vault.unseal(pin)) {
      if (context.mounted) _toast(context, 'Incorrect PIN');
      return;
    }
    await ref.read(appLockProvider.notifier).clearPin();
  }

  void _toast(BuildContext context, String message) =>
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(message)));
}

/// Collects a new PIN (with confirmation), plus the current PIN when changing.
/// Owns its controllers and pops `(oldPin, newPin)` or null on cancel.
class _PinSetupDialog extends StatefulWidget {
  const _PinSetupDialog({required this.isChange});
  final bool isChange;

  @override
  State<_PinSetupDialog> createState() => _PinSetupDialogState();
}

class _PinSetupDialogState extends State<_PinSetupDialog> {
  final _current = TextEditingController();
  final _pin = TextEditingController();
  final _confirm = TextEditingController();
  final _formKey = GlobalKey<FormState>();

  @override
  void dispose() {
    _current.dispose();
    _pin.dispose();
    _confirm.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(widget.isChange ? 'Change PIN' : 'Set a PIN'),
      content: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (widget.isChange)
              _pinField(_current, 'Current PIN', autofocus: true),
            _pinField(_pin, 'New PIN',
                autofocus: !widget.isChange,
                validator: (v) =>
                    (v == null || v.length < 4) ? 'Use at least 4 digits' : null),
            _pinField(_confirm, 'Confirm PIN',
                validator: (v) => v != _pin.text ? 'PINs do not match' : null),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () {
            if (_formKey.currentState!.validate()) {
              Navigator.of(context).pop((
                oldPin: widget.isChange ? _current.text : null,
                newPin: _pin.text,
              ));
            }
          },
          child: const Text('Save'),
        ),
      ],
    );
  }

  Widget _pinField(
    TextEditingController controller,
    String label, {
    bool autofocus = false,
    String? Function(String?)? validator,
  }) =>
      TextFormField(
        controller: controller,
        autofocus: autofocus,
        obscureText: true,
        keyboardType: TextInputType.number,
        inputFormatters: [FilteringTextInputFormatter.digitsOnly],
        decoration: InputDecoration(labelText: label),
        validator: validator,
      );
}
