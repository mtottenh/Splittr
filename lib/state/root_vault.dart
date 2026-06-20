import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';

import '../src/rust/api.dart' as ffi show sealRootSeed, openRootSeed;
import 'engine.dart';

/// Stores the identity (root) seed for privileged actions, sealed under the
/// app-lock PIN while app lock is on (#34/ADR-0005).
///
/// When app lock is **off**, the seed sits in the OS keystore in the clear (as
/// the device/db keys do). When it is **on**, the seed is encrypted with
/// `seal_root_seed` (Argon2id + AEAD) under the PIN and the plaintext copy is
/// removed, so a keystore/file leak alone can't expose the root — unsealing
/// needs the PIN. The recovery phrase remains the ultimate backup.
class RootVault {
  RootVault(this._dir);

  final Directory _dir;
  SecretStore get _store => SecretStore(_dir);

  /// Whether the root is currently PIN-sealed (i.e. app lock is on).
  Future<bool> isSealed() async => (await _store.readBlob(kRootVault)) != null;

  /// The root seed for a privileged action. When sealed, [pin] is required and
  /// a wrong/missing pin yields null; when not sealed, returns the plaintext.
  Future<List<int>?> loadSeed({String? pin}) async {
    final vault = await _store.readBlob(kRootVault);
    if (vault != null) {
      if (pin == null) return null;
      return ffi.openRootSeed(passphrase: pin, blob: vault);
    }
    return _store.read(kIdentitySeed);
  }

  /// Seal the plaintext root under [pin] and drop the plaintext copy. A no-op if
  /// there is no plaintext to seal (already sealed).
  Future<void> seal(String pin) async {
    final seed = await _store.read(kIdentitySeed);
    if (seed == null) return;
    final blob = await ffi.sealRootSeed(passphrase: pin, seed: seed);
    await _store.writeBlob(kRootVault, blob);
    await _store.delete(kIdentitySeed);
  }

  /// Re-seal under [newPin]; needs the current [oldPin]. Returns false if the
  /// old pin is wrong (so a PIN change can't silently lose the root).
  Future<bool> reseal({required String oldPin, required String newPin}) async {
    final vault = await _store.readBlob(kRootVault);
    if (vault == null) return false;
    final seed = await ffi.openRootSeed(passphrase: oldPin, blob: vault);
    if (seed == null) return false;
    final blob = await ffi.sealRootSeed(passphrase: newPin, seed: seed);
    await _store.writeBlob(kRootVault, blob);
    return true;
  }

  /// Restore the plaintext root from the vault (turning app lock off); needs
  /// [pin]. Returns false if the pin is wrong.
  Future<bool> unseal(String pin) async {
    final vault = await _store.readBlob(kRootVault);
    if (vault == null) return true; // already plaintext
    final seed = await ffi.openRootSeed(passphrase: pin, blob: vault);
    if (seed == null) return false;
    await _store.write(kIdentitySeed, seed);
    await _store.delete(kRootVault);
    return true;
  }
}

final rootVaultProvider = FutureProvider<RootVault>((ref) async {
  await ref.watch(rustInitProvider.future);
  final dir = await getApplicationSupportDirectory();
  return RootVault(dir);
});
