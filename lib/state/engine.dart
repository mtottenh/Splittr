import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:path_provider/path_provider.dart';

import '../src/rust/api.dart';
import '../src/rust/api.dart' as ffi
    show recoveryPhrase, seedFromRecoveryPhrase;
import '../src/rust/frb_generated.dart';

/// Keystore / fallback-file names for the local key material.
const kIdentitySeed = ('splittr_identity_seed', 'identity.seed');

/// The identity seed sealed under the app-lock PIN (#34); present only while app
/// lock is on. A variable-length AEAD blob, so it bypasses the 32-byte helpers.
const kRootVault = ('splittr_root_vault', 'root.vault');
const _kIdentityPublic = ('splittr_identity_public', 'identity.pub');
const _kDeviceSeed = ('splittr_device_seed', 'device.seed');
const _kDbKey = ('splittr_db_key', 'db.key');

/// Opens the Rust [Engine] exactly once for the app.
///
/// This is the single seam between the Flutter shell and `splittr-core`. Daily
/// launches open with the identity (root) key **locked** (#34/ADR-0005): only
/// the device key is loaded, so routine use never touches the root. The root is
/// unlocked on demand for privileged actions (enrol/revoke a device). The very
/// first launch (or one right after a recovery-phrase restore) opens with the
/// root so it can self-enrol this device, then records the identity public key
/// for subsequent device-only launches.
/// Initialises the Rust bridge exactly once (loading the bundled native lib).
/// Shared so onboarding and the engine don't double-init the FFI.
final rustInitProvider = FutureProvider<void>((ref) => RustLib.init());

final engineProvider = FutureProvider<Engine>((ref) async {
  await ref.watch(rustInitProvider.future);
  final dir = await getApplicationSupportDirectory();
  final secrets = SecretStore(dir);

  // The device seed (#16) and db key (#22) are always needed; the site (HLC
  // tiebreaker) is per-device, so derive it from the device seed.
  final deviceSeed = await secrets.loadOrCreate(_kDeviceSeed);
  final dbKey = await secrets.loadOrCreate(_kDbKey);
  final site = _siteFromSeed(deviceSeed);
  final dbPath = '${dir.path}/splittr.redb';

  final identityPublic = await secrets.read(_kIdentityPublic);
  if (identityPublic != null) {
    return Engine.openDeviceOnly(
      dbPath: dbPath,
      identityPublic: identityPublic,
      deviceSeed: deviceSeed,
      dbKey: dbKey,
      site: site,
    );
  }

  // First run: the identity seed must exist (created here, or written by the
  // restore-from-phrase onboarding). A full open self-enrols this device.
  final identitySeed = await secrets.loadOrCreate(kIdentitySeed);
  final engine = await Engine.open(
    dbPath: dbPath,
    identitySeed: identitySeed,
    deviceSeed: deviceSeed,
    dbKey: dbKey,
    site: site,
  );
  await secrets.write(
      _kIdentityPublic, _hexToBytes(await engine.identityPublic()));
  return engine;
});

/// Loads/persists 32-byte key material, preferring the OS keystore and falling
/// back to a local file where secure storage is unavailable (e.g. a headless
/// desktop with no secret service). The fallback still keeps the db encrypted;
/// hardening that last gap is tracked in #6.
class SecretStore {
  SecretStore(this._dir);

  final Directory _dir;
  final FlutterSecureStorage _keystore = const FlutterSecureStorage(
    aOptions: AndroidOptions(encryptedSharedPreferences: true),
  );

  /// Read a stored 32-byte value, or `null` if it has never been written.
  Future<List<int>?> read((String, String) key) async {
    try {
      final existing = await _keystore.read(key: key.$1);
      if (existing != null) {
        final bytes = base64Decode(existing);
        if (bytes.length == 32) return bytes;
      }
    } catch (_) {
      // Fall through to the file fallback.
    }
    final file = File('${_dir.path}/${key.$2}');
    if (file.existsSync()) {
      final bytes = await file.readAsBytes();
      if (bytes.length == 32) return bytes;
    }
    return null;
  }

  /// Persist a 32-byte value to the keystore, or the file fallback if there is
  /// no secret service.
  Future<void> write((String, String) key, List<int> value) async {
    try {
      await _keystore.write(key: key.$1, value: base64Encode(value));
    } catch (_) {
      await File('${_dir.path}/${key.$2}').writeAsBytes(value, flush: true);
    }
  }

  /// Read the value, creating a fresh random one the first time.
  Future<List<int>> loadOrCreate((String, String) key) async {
    final existing = await read(key);
    if (existing != null) return existing;
    final fresh = _random32();
    await write(key, fresh);
    return fresh;
  }

  /// Read a variable-length blob (e.g. the sealed root vault), or null.
  Future<List<int>?> readBlob((String, String) key) async {
    try {
      final existing = await _keystore.read(key: key.$1);
      if (existing != null) return base64Decode(existing);
    } catch (_) {
      // Fall through to the file fallback.
    }
    final file = File('${_dir.path}/${key.$2}');
    if (file.existsSync()) return file.readAsBytes();
    return null;
  }

  /// Persist a variable-length blob.
  Future<void> writeBlob((String, String) key, List<int> bytes) async {
    try {
      await _keystore.write(key: key.$1, value: base64Encode(bytes));
    } catch (_) {
      await File('${_dir.path}/${key.$2}').writeAsBytes(bytes, flush: true);
    }
  }

  /// Remove a value from both the keystore and the file fallback.
  Future<void> delete((String, String) key) async {
    try {
      await _keystore.delete(key: key.$1);
    } catch (_) {
      // Ignore: no keystore, only the file fallback to clear.
    }
    final file = File('${_dir.path}/${key.$2}');
    if (file.existsSync()) file.deleteSync();
  }
}

Uint8List _random32() {
  final rng = Random.secure();
  return Uint8List.fromList(List.generate(32, (_) => rng.nextInt(256)));
}

List<int> _hexToBytes(String hex) => [
      for (var i = 0; i < hex.length; i += 2)
        int.parse(hex.substring(i, i + 2), radix: 16),
    ];

/// Derive a stable per-device HLC site id from the seed (the clock tiebreaker).
BigInt _siteFromSeed(List<int> seed) {
  var site = BigInt.zero;
  for (var i = 0; i < 8; i++) {
    site = (site << 8) | BigInt.from(seed[i]);
  }
  return site;
}

/// Whether an identity already exists on this device. `false` only on a truly
/// fresh install, which routes through onboarding (#34).
final identityEstablishedProvider = FutureProvider<bool>((ref) async {
  final dir = await getApplicationSupportDirectory();
  final secrets = SecretStore(dir);
  return (await secrets.read(_kIdentityPublic)) != null ||
      (await secrets.read(kIdentitySeed)) != null ||
      (await secrets.readBlob(kRootVault)) != null;
});

/// Establishes the local identity on first run: either a brand-new key (whose
/// recovery phrase is shown once) or one restored from a recovery phrase (#34).
abstract class IdentityBootstrap {
  /// Create a new identity and return its 24-word recovery phrase to show once.
  Future<String> createNew();

  /// Restore an identity from a recovery phrase. Returns false if invalid.
  Future<bool> restore(String phrase);
}

class _FileIdentityBootstrap implements IdentityBootstrap {
  _FileIdentityBootstrap(this._dir);
  final Directory _dir;

  @override
  Future<String> createNew() async {
    final seed = _random32();
    await SecretStore(_dir).write(kIdentitySeed, seed);
    return ffi.recoveryPhrase(identitySeed: seed);
  }

  @override
  Future<bool> restore(String phrase) async {
    final List<int> seed;
    try {
      seed = await ffi.seedFromRecoveryPhrase(phrase: phrase.trim());
    } catch (_) {
      return false; // not a valid BIP39 phrase for our identity
    }
    await SecretStore(_dir).write(kIdentitySeed, seed);
    return true;
  }
}

final identityBootstrapProvider = FutureProvider<IdentityBootstrap>((ref) async {
  await ref.watch(rustInitProvider.future);
  final dir = await getApplicationSupportDirectory();
  return _FileIdentityBootstrap(dir);
});
