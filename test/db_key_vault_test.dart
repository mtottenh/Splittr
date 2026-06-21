// Tests PIN-sealing of the op-log encryption key (§4a): app lock off keeps it in
// the clear; sealing removes the plaintext and requires the PIN to recover it;
// re-key and unseal behave; this mirrors how the engine obtains the key at open
// on a keystore-less platform. Runs over the real FFI (Argon2id + AEAD);
// SecretStore has no keystore under `flutter test`, so the keystore-less
// (file-fallback) path — exactly the one §4a is about — is exercised.

import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/src/rust/frb_generated.dart';
import 'package:splittr/state/engine.dart';
import 'package:splittr/state/pin_sealed_key.dart';

void main() {
  setUpAll(() async {
    final lib =
        '${Directory.current.path}/splittr-core/target/debug/libsplittr_ffi.so';
    await RustLib.init(externalLibrary: ExternalLibrary.open(lib));
  });

  PinSealedKey vaultFor(Directory dir) => PinSealedKey(
        dir,
        plainKey: kDbKey,
        vaultKey: kDbVault,
        skipWhenKeystore: true,
      );

  test('seals the db key under the PIN and unseals it for engine open',
      () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_dbvault');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final store = SecretStore(tmp);
    final vault = vaultFor(tmp);
    final key = List.filled(32, 9);

    // App lock off: the key is plaintext and loads directly.
    await store.write(kDbKey, key);
    expect(await vault.isSealed(), isFalse);
    expect(await vault.load(), key);

    // Sealing under the PIN drops the plaintext file; the key now needs the PIN.
    await vault.seal('1234');
    expect(await vault.isSealed(), isTrue);
    expect(await store.read(kDbKey), isNull, reason: 'plaintext removed');
    expect(await vault.load(), isNull, reason: 'no PIN supplied');
    expect(await vault.load(pin: '0000'), isNull, reason: 'wrong PIN');
    expect(await vault.load(pin: '1234'), key);

    // Changing the PIN re-keys the vault; the wrong current PIN fails.
    expect(await vault.reseal(oldPin: '0000', newPin: '5678'), isFalse);
    expect(await vault.reseal(oldPin: '1234', newPin: '5678'), isTrue);
    expect(await vault.load(pin: '1234'), isNull);
    expect(await vault.load(pin: '5678'), key);

    // Turning the lock off needs the PIN and restores the plaintext.
    expect(await vault.unseal('0000'), isFalse);
    expect(await vault.unseal('5678'), isTrue);
    expect(await vault.isSealed(), isFalse);
    expect(await vault.load(), key);
  });

  test('reseal/unseal are no-op successes when nothing is sealed', () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_dbvault2');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final vault = vaultFor(tmp);
    // No blob (e.g. a keystore platform where sealing is skipped): a PIN change
    // or removal must not report a spurious failure.
    expect(await vault.reseal(oldPin: '1', newPin: '2'), isTrue);
    expect(await vault.unseal('1'), isTrue);
  });

  test('loadOrCreate mints a key once, then is stable', () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_dbvault3');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final vault = vaultFor(tmp);
    final created = await vault.loadOrCreate();
    expect(created, hasLength(32));
    expect(await vault.loadOrCreate(), created, reason: 'stable across calls');
  });
}
