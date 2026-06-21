import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';

import 'engine.dart';
import 'pin_sealed_key.dart';

/// Stores the identity (root) seed for privileged actions, sealed under the
/// app-lock PIN while app lock is on (#34/ADR-0005).
///
/// A thin wrapper over [PinSealedKey] for the identity seed: when app lock is
/// off the seed sits in the keystore in the clear (as it always has); when on it
/// is Argon2id+AEAD-sealed under the PIN with the plaintext removed, so a
/// keystore/file leak alone can't expose the root. The recovery phrase remains
/// the ultimate backup. The root is needed only for privileged actions, so it is
/// always PIN-sealed when locked (unlike the db key, it has no keystore skip).
class RootVault {
  RootVault(Directory dir)
      : _key = PinSealedKey(
          dir,
          plainKey: kIdentitySeed,
          vaultKey: kRootVault,
        );

  final PinSealedKey _key;

  /// Whether the root is currently PIN-sealed (i.e. app lock is on).
  Future<bool> isSealed() => _key.isSealed();

  /// The root seed for a privileged action. When sealed, [pin] is required and a
  /// wrong/missing pin yields null; when not sealed, returns the plaintext.
  Future<List<int>?> loadSeed({String? pin}) => _key.load(pin: pin);

  /// Seal the plaintext root under [pin] and drop the plaintext copy. A no-op if
  /// there is no plaintext to seal (already sealed).
  Future<void> seal(String pin) => _key.seal(pin);

  /// Re-seal under [newPin]; needs the current [oldPin]. Returns false if the
  /// old pin is wrong (so a PIN change can't silently lose the root).
  Future<bool> reseal({required String oldPin, required String newPin}) =>
      _key.reseal(oldPin: oldPin, newPin: newPin);

  /// Restore the plaintext root from the vault (turning app lock off); needs
  /// [pin]. Returns false if the pin is wrong.
  Future<bool> unseal(String pin) => _key.unseal(pin);
}

final rootVaultProvider = FutureProvider<RootVault>((ref) async {
  await ref.watch(rustInitProvider.future);
  final dir = await getApplicationSupportDirectory();
  return RootVault(dir);
});
