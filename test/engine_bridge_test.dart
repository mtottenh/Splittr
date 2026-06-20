// Proves the Flutter ↔ Rust bridge: loads the host-built engine library and
// drives the real Rust engine end-to-end through the generated FFI bindings.
//
// Requires the native lib to be built first:
//   (cd splittr-core && cargo build -p splittr-ffi)
// In production the library is built + bundled by the platform build (cargokit);
// here we load it explicitly so the bridge can be exercised under `flutter test`.

import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/src/rust/api.dart';
import 'package:splittr/src/rust/dto.dart';
import 'package:splittr/src/rust/frb_generated.dart';

void main() {
  setUpAll(() async {
    final lib =
        '${Directory.current.path}/splittr-core/target/debug/libsplittr_ffi.so';
    await RustLib.init(externalLibrary: ExternalLibrary.open(lib));
  });

  test('drives the Rust engine end-to-end through the FFI', () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_bridge');
    addTearDown(() => tmp.deleteSync(recursive: true));

    final engine = await Engine.open(
      dbPath: '${tmp.path}/engine.redb',
      identitySeed: List.filled(32, 7),
      site: BigInt.one,
    );

    await engine.setMyName(name: 'Me');
    final me = await engine.myUserId();
    final bob = await engine.addPerson(name: 'Bob');
    final group = await engine.createGroup(name: 'Trip', memberIds: [bob]);

    await engine.addExpense(
      input: ExpenseInput(
        groupId: group,
        description: 'Hotel',
        paidBy: [Payer(userId: me, cents: 3000)],
        totalCents: 3000,
        split: SplitPlanDto.equal(participants: [me, bob]),
        category: 'travel',
        dateMs: 0,
      ),
    );

    final detail = (await engine.groupDetail(groupId: group))!;
    expect(detail.name, 'Trip');
    expect(detail.members.firstWhere((m) => m.userId == me).netCents, 1500);
    expect(detail.members.firstWhere((m) => m.userId == bob).netCents, -1500);
    expect(detail.settleUp, hasLength(1));
    expect(detail.settleUp.first.amountCents, 1500);

    await engine.recordSettlement(
      groupId: group,
      from: bob,
      to: me,
      amountCents: 1500,
    );
    final settled = (await engine.groupDetail(groupId: group))!;
    expect(settled.settleUp, isEmpty);
    expect(settled.members.every((m) => m.netCents == 0), isTrue);
  });
}
