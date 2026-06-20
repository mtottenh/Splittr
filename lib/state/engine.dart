import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path_provider/path_provider.dart';

import '../src/rust/api.dart';
import '../src/rust/frb_generated.dart';

/// Opens the Rust [Engine] exactly once for the app.
///
/// This is the single seam between the Flutter shell and `splittr-core`: it
/// initialises the FFI, loads (or creates) the local signing identity, and opens
/// the redb-backed op-log. Everything downstream is presentation.
final engineProvider = FutureProvider<Engine>((ref) async {
  await RustLib.init();
  final dir = await getApplicationSupportDirectory();
  final seed = await _loadOrCreateSeed(File('${dir.path}/identity.seed'));
  return Engine.open(
    dbPath: '${dir.path}/splittr.redb',
    identitySeed: seed,
    site: _siteFromSeed(seed),
  );
});

/// The 32-byte identity seed *is* the user's key material. It is persisted
/// locally; secure-storage hardening is a later concern (#16/#22).
Future<List<int>> _loadOrCreateSeed(File file) async {
  if (file.existsSync()) {
    final bytes = await file.readAsBytes();
    if (bytes.length == 32) return bytes;
  }
  final rng = Random.secure();
  final seed = Uint8List.fromList(List.generate(32, (_) => rng.nextInt(256)));
  await file.writeAsBytes(seed, flush: true);
  return seed;
}

/// Derive a stable per-device HLC site id from the seed (the clock tiebreaker).
BigInt _siteFromSeed(List<int> seed) {
  var site = BigInt.zero;
  for (var i = 0; i < 8; i++) {
    site = (site << 8) | BigInt.from(seed[i]);
  }
  return site;
}
