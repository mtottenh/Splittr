// Tests the PIN-sealed root vault (#34): sealing removes the plaintext, only the
// right PIN unseals, changing the PIN re-keys it, and removing the lock restores
// the plaintext. Runs over the real FFI (Argon2id + AEAD); SecretStore falls
// back to files since there is no keystore under `flutter test`.

import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/src/rust/frb_generated.dart';
import 'package:splittr/state/engine.dart';
import 'package:splittr/state/root_vault.dart';

void main() {
  setUpAll(() async {
    final lib =
        '${Directory.current.path}/splittr-core/target/debug/libsplittr_ffi.so';
    await RustLib.init(externalLibrary: ExternalLibrary.open(lib));
  });

  test('seals, re-keys and unseals the root under the PIN', () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_vault');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final store = SecretStore(tmp);
    final vault = RootVault(tmp);
    final seed = List.filled(32, 5);

    // App lock off: the seed is plaintext and loads directly.
    await store.write(kIdentitySeed, seed);
    expect(await vault.isSealed(), isFalse);
    expect(await vault.loadSeed(), seed);

    // Sealing under a PIN drops the plaintext; the seed needs the PIN.
    await vault.seal('1234');
    expect(await vault.isSealed(), isTrue);
    expect(await store.read(kIdentitySeed), isNull);
    expect(await vault.loadSeed(), isNull); // no PIN supplied
    expect(await vault.loadSeed(pin: '0000'), isNull); // wrong PIN
    expect(await vault.loadSeed(pin: '1234'), seed);

    // Changing the PIN re-keys the vault and needs the current PIN.
    expect(await vault.reseal(oldPin: '0000', newPin: '5678'), isFalse);
    expect(await vault.reseal(oldPin: '1234', newPin: '5678'), isTrue);
    expect(await vault.loadSeed(pin: '1234'), isNull);
    expect(await vault.loadSeed(pin: '5678'), seed);

    // Removing the lock needs the PIN and restores the plaintext.
    expect(await vault.unseal('0000'), isFalse);
    expect(await vault.unseal('5678'), isTrue);
    expect(await vault.isSealed(), isFalse);
    expect(await vault.loadSeed(), seed);
  });
}
