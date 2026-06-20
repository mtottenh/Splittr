import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/app_lock.dart';
import '../state/root_vault.dart';

/// Shown when the app is locked (#22): unlock with biometrics (if available) or
/// the PIN.
class LockScreen extends ConsumerStatefulWidget {
  const LockScreen({super.key});

  @override
  ConsumerState<LockScreen> createState() => _LockScreenState();
}

class _LockScreenState extends ConsumerState<LockScreen> {
  final _pin = TextEditingController();
  bool _error = false;
  bool _biometricsAvailable = false;

  @override
  void initState() {
    super.initState();
    // Offer biometrics immediately when supported.
    Future.microtask(() async {
      final available =
          await ref.read(appLockProvider.notifier).biometricsAvailable();
      if (!mounted) return;
      setState(() => _biometricsAvailable = available);
      if (available) {
        await ref.read(appLockProvider.notifier).unlockWithBiometrics();
      }
    });
  }

  @override
  void dispose() {
    _pin.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final pin = _pin.text;
    final ok = await ref.read(appLockProvider.notifier).unlockWithPin(pin);
    if (!ok) {
      setState(() => _error = true);
      _pin.clear();
      return;
    }
    // Migrate installs whose PIN predates root sealing: now that we hold the
    // PIN, seal the still-plaintext root under it (a no-op once sealed) (#34).
    final vault = await ref.read(rootVaultProvider.future);
    await vault.seal(pin);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 320),
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(Icons.lock_outline,
                    size: 56, color: theme.colorScheme.primary),
                const SizedBox(height: 16),
                Text('Splittr is locked', style: theme.textTheme.titleLarge),
                const SizedBox(height: 24),
                TextField(
                  controller: _pin,
                  autofocus: true,
                  obscureText: true,
                  keyboardType: TextInputType.number,
                  textAlign: TextAlign.center,
                  inputFormatters: [
                    FilteringTextInputFormatter.digitsOnly,
                  ],
                  decoration: InputDecoration(
                    labelText: 'PIN',
                    errorText: _error ? 'Incorrect PIN' : null,
                  ),
                  onChanged: (_) {
                    if (_error) setState(() => _error = false);
                  },
                  onSubmitted: (_) => _submit(),
                ),
                const SizedBox(height: 16),
                FilledButton(
                  onPressed: _submit,
                  child: const Text('Unlock'),
                ),
                if (_biometricsAvailable) ...[
                  const SizedBox(height: 8),
                  TextButton.icon(
                    onPressed: () =>
                        ref.read(appLockProvider.notifier).unlockWithBiometrics(),
                    icon: const Icon(Icons.fingerprint),
                    label: const Text('Use biometrics'),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}
