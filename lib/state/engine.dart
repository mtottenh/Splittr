import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:path_provider/path_provider.dart';

import '../src/rust/api.dart';
import '../src/rust/frb_generated.dart';

/// Opens the Rust [Engine] exactly once for the app.
///
/// This is the single seam between the Flutter shell and `splittr-core`: it
/// initialises the FFI, loads (or creates) the local signing identity and the
/// at-rest encryption key, and opens the encrypted redb-backed op-log.
final engineProvider = FutureProvider<Engine>((ref) async {
  await RustLib.init();
  final dir = await getApplicationSupportDirectory();
  final secrets = _SecretStore(dir);

  // The identity seed is the user's root key material (#6); the device seed is
  // this device's own key (#16); the db key encrypts the op-log at rest (#22).
  // All live in the OS keystore, never next to the db. The site (HLC tiebreaker)
  // is per-device, so derive it from the device seed.
  final seed = await secrets.loadOrCreate('splittr_identity_seed', 'identity.seed');
  final deviceSeed =
      await secrets.loadOrCreate('splittr_device_seed', 'device.seed');
  final dbKey = await secrets.loadOrCreate('splittr_db_key', 'db.key');

  return Engine.open(
    dbPath: '${dir.path}/splittr.redb',
    identitySeed: seed,
    deviceSeed: deviceSeed,
    dbKey: dbKey,
    site: _siteFromSeed(deviceSeed),
  );
});

/// Loads the local identity seed (for deriving the recovery phrase, #34).
final identitySeedProvider = FutureProvider<List<int>>((ref) async {
  final dir = await getApplicationSupportDirectory();
  return _SecretStore(dir).loadOrCreate('splittr_identity_seed', 'identity.seed');
});

/// Loads/persists 32-byte secrets, preferring the OS keystore and falling back
/// to a local file where secure storage is unavailable (e.g. a headless desktop
/// with no secret service). The fallback still keeps the db encrypted; hardening
/// that last gap is tracked in #6.
class _SecretStore {
  _SecretStore(this._dir);

  final Directory _dir;
  final FlutterSecureStorage _keystore = const FlutterSecureStorage(
    aOptions: AndroidOptions(encryptedSharedPreferences: true),
  );

  Future<List<int>> loadOrCreate(String key, String fallbackFile) async {
    try {
      final existing = await _keystore.read(key: key);
      if (existing != null) {
        final bytes = base64Decode(existing);
        if (bytes.length == 32) return bytes;
      }
      final fresh = _random32();
      await _keystore.write(key: key, value: base64Encode(fresh));
      return fresh;
    } catch (_) {
      return _fileLoadOrCreate(File('${_dir.path}/$fallbackFile'));
    }
  }

  Future<List<int>> _fileLoadOrCreate(File file) async {
    if (file.existsSync()) {
      final bytes = await file.readAsBytes();
      if (bytes.length == 32) return bytes;
    }
    final fresh = _random32();
    await file.writeAsBytes(fresh, flush: true);
    return fresh;
  }
}

Uint8List _random32() {
  final rng = Random.secure();
  return Uint8List.fromList(List.generate(32, (_) => rng.nextInt(256)));
}

/// Derive a stable per-device HLC site id from the seed (the clock tiebreaker).
BigInt _siteFromSeed(List<int> seed) {
  var site = BigInt.zero;
  for (var i = 0; i < 8; i++) {
    site = (site << 8) | BigInt.from(seed[i]);
  }
  return site;
}
