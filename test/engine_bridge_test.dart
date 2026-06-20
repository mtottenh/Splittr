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
      dbKey: List.filled(32, 11),
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

  test('exposes the friends, activity and detail queries the UI binds to',
      () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_bridge_q');
    addTearDown(() => tmp.deleteSync(recursive: true));

    final engine = await Engine.open(
      dbPath: '${tmp.path}/engine.redb',
      identitySeed: List.filled(32, 9),
      dbKey: List.filled(32, 13),
      site: BigInt.two,
    );

    await engine.setMyName(name: 'Me');
    expect(await engine.myName(), 'Me');
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

    // Friends list carries the per-friend balance (Bob owes Me 1500).
    final friends = await engine.friends();
    final bobFriend = friends.firstWhere((f) => f.userId == bob);
    expect(bobFriend.name, 'Bob');
    expect(bobFriend.netCents, 1500);

    // Friend detail surfaces the shared expense with its group name.
    final detail = (await engine.friendDetail(userId: bob))!;
    expect(detail.netCents, 1500);
    expect(detail.shared, hasLength(1));
    expect(detail.shared.first.description, 'Hotel');
    expect(detail.shared.first.groupName, 'Trip');

    // The enriched expense view carries payers and splits for the editor.
    final expense = (await engine.groupDetail(groupId: group))!.expenses.first;
    expect(expense.paidBy, hasLength(1));
    expect(expense.paidBy.first.cents, 3000);
    expect(expense.splits, hasLength(2));

    // Activity feed is derived from the op-log.
    final activity = await engine.activity();
    expect(activity.any((a) => a.kind == 'expense'), isTrue);
    expect(activity.any((a) => a.kind == 'group'), isTrue);

    // Settling clears the friend balance and lands in the ledger.
    await engine.recordSettlement(
      groupId: group,
      from: bob,
      to: me,
      amountCents: 1500,
    );
    final after = await engine.friends();
    expect(after.firstWhere((f) => f.userId == bob).netCents, 0);
    expect((await engine.groupDetail(groupId: group))!.settlements, hasLength(1));
  });
}
