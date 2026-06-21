import 'dart:convert';
import 'dart:math';

import 'package:crypto/crypto.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:local_auth/local_auth.dart';

import 'engine.dart';

/// Salted SHA-256 of a PIN. Pure (no I/O) so it is unit-testable; the salt makes
/// the stored hash resistant to precomputation.
String hashPin(String pin, String salt) =>
    sha256.convert(utf8.encode('$salt:$pin')).toString();

enum LockStatus { unknown, locked, unlocked }

class AppLockState {
  const AppLockState({required this.status, required this.pinSet});
  final LockStatus status;
  final bool pinSet;

  AppLockState copyWith({LockStatus? status, bool? pinSet}) => AppLockState(
        status: status ?? this.status,
        pinSet: pinSet ?? this.pinSet,
      );
}

/// Biometric- / PIN-based app lock (#22). The PIN hash + salt live in the OS
/// keystore; biometrics use `local_auth` where available and degrade to PIN
/// (or no lock) elsewhere.
class AppLockController extends Notifier<AppLockState> {
  static const _hashKey = 'splittr_pin_hash';
  static const _saltKey = 'splittr_pin_salt';

  final FlutterSecureStorage _storage = const FlutterSecureStorage(
    aOptions: AndroidOptions(encryptedSharedPreferences: true),
  );
  final LocalAuthentication _auth = LocalAuthentication();

  @override
  AppLockState build() {
    Future.microtask(_bootstrap);
    return const AppLockState(status: LockStatus.unknown, pinSet: false);
  }

  Future<void> _bootstrap() async {
    final hash = await _read(_hashKey);
    state = hash == null
        ? const AppLockState(status: LockStatus.unlocked, pinSet: false)
        : const AppLockState(status: LockStatus.locked, pinSet: true);
  }

  /// Verify a PIN and unlock on success.
  Future<bool> unlockWithPin(String pin) async {
    final hash = await _read(_hashKey);
    final salt = await _read(_saltKey);
    if (hash == null || salt == null) {
      state = state.copyWith(status: LockStatus.unlocked, pinSet: false);
      return true;
    }
    if (hashPin(pin, salt) == hash) {
      // PIN verified — make the (possibly sealed) db key available before the
      // shell builds and opens the engine (§4a).
      await _prepareDbKey(pin);
      state = state.copyWith(status: LockStatus.unlocked);
      return true;
    }
    return false;
  }

  /// With the PIN known-correct: seal the db key under it if it isn't already
  /// (migrating a keystore-less plaintext key), then hold the unsealed key in
  /// memory for [engineProvider]. A no-op where a keystore protects the key.
  Future<void> _prepareDbKey(String pin) async {
    final vault = await ref.read(dbKeyVaultProvider.future);
    await vault.seal(pin);
    if (await vault.isSealed()) {
      ref.read(unlockedDbKeyProvider.notifier).state =
          await vault.load(pin: pin);
    }
  }

  /// Attempt a biometric / device-credential unlock. Returns false (without
  /// throwing) where unsupported.
  Future<bool> unlockWithBiometrics() async {
    if (!await biometricsAvailable()) return false;
    try {
      final ok = await _auth.authenticate(
        localizedReason: 'Unlock Splittr',
        options: const AuthenticationOptions(stickyAuth: true),
      );
      if (ok) state = state.copyWith(status: LockStatus.unlocked);
      return ok;
    } catch (_) {
      return false;
    }
  }

  Future<bool> biometricsAvailable() async {
    try {
      return await _auth.isDeviceSupported() &&
          (await _auth.getAvailableBiometrics()).isNotEmpty;
    } catch (_) {
      return false;
    }
  }

  /// Set (or change) the PIN; leaves the app unlocked.
  Future<void> setPin(String pin) async {
    final salt = _randomSalt();
    await _storage.write(key: _saltKey, value: salt);
    await _storage.write(key: _hashKey, value: hashPin(pin, salt));
    state = state.copyWith(status: LockStatus.unlocked, pinSet: true);
  }

  Future<void> clearPin() async {
    await _storage.delete(key: _hashKey);
    await _storage.delete(key: _saltKey);
    state = state.copyWith(status: LockStatus.unlocked, pinSet: false);
  }

  /// Re-lock (e.g. when the app is backgrounded), only if a PIN is configured.
  void lock() {
    if (state.pinSet) state = state.copyWith(status: LockStatus.locked);
  }

  Future<String?> _read(String key) async {
    try {
      return await _storage.read(key: key);
    } catch (_) {
      return null; // no keystore (e.g. headless) → treat as no PIN
    }
  }

  String _randomSalt() {
    final rng = Random.secure();
    return base64Encode(List.generate(16, (_) => rng.nextInt(256)));
  }
}

final appLockProvider =
    NotifierProvider<AppLockController, AppLockState>(AppLockController.new);
