import 'dart:io';

import '../src/rust/api.dart' as ffi show sealRootSeed, openRootSeed;
import 'engine.dart';

/// A 32-byte secret that can be sealed under the app-lock PIN (Argon2id + AEAD,
/// the same `seal_root_seed`/`open_root_seed` path as #34/ADR-0005).
///
/// When **not sealed** the secret sits in [SecretStore] (the OS keystore, or a
/// file fallback where there is no keystore). When **sealed** only the encrypted
/// blob is kept and the plaintext copy is removed, so a storage leak alone can't
/// expose it — unsealing needs the PIN.
///
/// [skipWhenKeystore] is for secrets that are needed at *every* launch (the db
/// key): on a platform with a keystore the OS already protects them and gates
/// release behind device unlock/biometrics, so sealing under the PIN there would
/// break a biometric unlock (no PIN available). Sealing is therefore applied
/// only on keystore-less platforms — exactly where the plaintext file fallback
/// would otherwise defeat at-rest encryption.
class PinSealedKey {
  PinSealedKey(
    this._dir, {
    required this.plainKey,
    required this.vaultKey,
    this.skipWhenKeystore = false,
  });

  final Directory _dir;
  final (String, String) plainKey;
  final (String, String) vaultKey;
  final bool skipWhenKeystore;

  SecretStore get _store => SecretStore(_dir);

  /// Whether the secret is currently PIN-sealed.
  Future<bool> isSealed() async => (await _store.readBlob(vaultKey)) != null;

  /// Load the secret. When sealed, [pin] is required and a wrong/missing pin
  /// yields null; otherwise the stored plaintext (or null if never created).
  Future<List<int>?> load({String? pin}) async {
    final blob = await _store.readBlob(vaultKey);
    if (blob != null) {
      if (pin == null) return null;
      return ffi.openRootSeed(passphrase: pin, blob: blob);
    }
    return _store.read(plainKey);
  }

  /// Like [load] but creates a fresh random secret when none exists and it is
  /// not sealed. (A sealed secret must already exist; pass the [pin].)
  Future<List<int>> loadOrCreate({String? pin}) async {
    final existing = await load(pin: pin);
    if (existing != null) return existing;
    return _store.loadOrCreate(plainKey);
  }

  /// Seal the plaintext under [pin] and drop the plaintext copy. A no-op when
  /// there is nothing to seal, or (for [skipWhenKeystore]) when a keystore is
  /// available to protect the secret instead.
  Future<void> seal(String pin) async {
    if (skipWhenKeystore && await _store.hasKeystore()) return;
    final secret = await _store.read(plainKey);
    if (secret == null) return; // already sealed, or nothing stored
    final blob = await ffi.sealRootSeed(passphrase: pin, seed: secret);
    await _store.writeBlob(vaultKey, blob);
    await _store.delete(plainKey);
  }

  /// Re-seal under [newPin], verifying [oldPin]. Returns true on success, or when
  /// there is no sealed blob (nothing to re-key — e.g. sealing was skipped); only
  /// a wrong old pin returns false.
  Future<bool> reseal({required String oldPin, required String newPin}) async {
    final blob = await _store.readBlob(vaultKey);
    if (blob == null) return true; // nothing sealed on this platform
    final secret = await ffi.openRootSeed(passphrase: oldPin, blob: blob);
    if (secret == null) return false;
    await _store.writeBlob(
        vaultKey, await ffi.sealRootSeed(passphrase: newPin, seed: secret));
    return true;
  }

  /// Restore the plaintext (turning app lock off), verifying [pin]. Returns false
  /// only on a wrong pin.
  Future<bool> unseal(String pin) async {
    final blob = await _store.readBlob(vaultKey);
    if (blob == null) return true; // already plaintext
    final secret = await ffi.openRootSeed(passphrase: pin, blob: blob);
    if (secret == null) return false;
    await _store.write(plainKey, secret);
    await _store.delete(vaultKey);
    return true;
  }
}
