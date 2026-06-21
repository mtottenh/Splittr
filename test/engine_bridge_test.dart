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
      deviceSeed: List.filled(32, 7 + 100),
      dbKey: List.filled(32, 11),
      site: BigInt.one,
    );

    await engine.setMyName(name: 'Me');
    final me = await engine.myUserId();
    final bob = await engine.addPerson(name: 'Bob');
    final group = await engine.createGroup(name: 'Trip', memberIds: [bob], currency: 'USD');

    await engine.addExpense(
      input: ExpenseInput(
        groupId: group,
        description: 'Hotel',
        paidBy: [Payer(userId: me, cents: 3000)],
        totalCents: 3000,
        split: SplitPlanDto.equal(participants: [me, bob]),
        category: 'travel',
        dateMs: 0,
        draft: false,
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
      deviceSeed: List.filled(32, 9 + 100),
      dbKey: List.filled(32, 13),
      site: BigInt.two,
    );

    await engine.setMyName(name: 'Me');
    expect(await engine.myName(), 'Me');
    final me = await engine.myUserId();
    final bob = await engine.addPerson(name: 'Bob');
    final group = await engine.createGroup(name: 'Trip', memberIds: [bob], currency: 'USD');

    await engine.addExpense(
      input: ExpenseInput(
        groupId: group,
        description: 'Hotel',
        paidBy: [Payer(userId: me, cents: 3000)],
        totalCents: 3000,
        split: SplitPlanDto.equal(participants: [me, bob]),
        category: 'travel',
        dateMs: 0,
        draft: false,
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

  test('multi-currency: convert + stored base amount with original metadata',
      () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_bridge_fx');
    addTearDown(() => tmp.deleteSync(recursive: true));

    final engine = await Engine.open(
      dbPath: '${tmp.path}/engine.redb',
      identitySeed: List.filled(32, 41),
      deviceSeed: List.filled(32, 41 + 100),
      dbKey: List.filled(32, 43),
      site: BigInt.from(6),
    );
    await engine.setMyName(name: 'Me');
    final me = await engine.myUserId();
    final bob = await engine.addPerson(name: 'Bob');
    final group =
        await engine.createGroup(name: 'Iceland', memberIds: [bob], currency: 'EUR');

    // €30.00 entered as $32.40 at 1 USD = 0.925926 EUR.
    final baseCents = await convertCurrency(
      amountCents: 3240,
      rateMicro: 925926,
      fromCurrency: 'USD',
      toCurrency: 'EUR',
    );
    expect(baseCents, 3000);

    await engine.addExpense(
      input: ExpenseInput(
        groupId: group,
        description: 'Hotel',
        paidBy: [Payer(userId: me, cents: baseCents)],
        totalCents: baseCents,
        split: SplitPlanDto.equal(participants: [me, bob]),
        category: 'travel',
        dateMs: 0,
        draft: false,
        original: const OriginalAmountDto(
          currency: 'USD',
          amountCents: 3240,
          rateMicro: 925926,
        ),
      ),
    );

    final detail = (await engine.groupDetail(groupId: group))!;
    expect(detail.currency, 'EUR');
    final expense = detail.expenses.single;
    expect(expense.currency, 'EUR');
    expect(expense.totalCents, 3000);
    expect(expense.original?.currency, 'USD');
    expect(expense.original?.amountCents, 3240);
  });

  test('multiple payers are attributed through the FFI', () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_bridge_mp');
    addTearDown(() => tmp.deleteSync(recursive: true));

    final engine = await Engine.open(
      dbPath: '${tmp.path}/engine.redb',
      identitySeed: List.filled(32, 31),
      deviceSeed: List.filled(32, 31 + 100),
      dbKey: List.filled(32, 37),
      site: BigInt.from(5),
    );
    await engine.setMyName(name: 'Me');
    final me = await engine.myUserId();
    final bob = await engine.addPerson(name: 'Bob');
    final group = await engine.createGroup(name: 'Trip', memberIds: [bob], currency: 'USD');

    // A $60 dinner: I paid $40, Bob paid $20; split equally ($30 each).
    await engine.addExpense(
      input: ExpenseInput(
        groupId: group,
        description: 'Dinner',
        paidBy: [
          Payer(userId: me, cents: 4000),
          Payer(userId: bob, cents: 2000),
        ],
        totalCents: 6000,
        split: SplitPlanDto.equal(participants: [me, bob]),
        category: 'food',
        dateMs: 0,
        draft: false,
      ),
    );

    // I paid 40, owe 30 → net +10; Bob paid 20, owes 30 → net -10.
    final detail = (await engine.groupDetail(groupId: group))!;
    expect(detail.expenses.single.paidBy, hasLength(2));
    int net(String u) =>
        detail.members.firstWhere((m) => m.userId == u).netCents;
    expect(net(me), 1000);
    expect(net(bob), -1000);
  });

  test('non-group expenses flow through the FFI', () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_bridge_ng');
    addTearDown(() => tmp.deleteSync(recursive: true));

    final engine = await Engine.open(
      dbPath: '${tmp.path}/engine.redb',
      identitySeed: List.filled(32, 23),
      deviceSeed: List.filled(32, 23 + 100),
      dbKey: List.filled(32, 29),
      site: BigInt.from(4),
    );
    await engine.setMyName(name: 'Me');
    final me = await engine.myUserId();
    final bob = await engine.addPerson(name: 'Bob');

    // No group: I pay 20.00 split with Bob.
    await engine.addExpense(
      input: ExpenseInput(
        groupId: null,
        description: 'Coffee',
        paidBy: [Payer(userId: me, cents: 2000)],
        totalCents: 2000,
        split: SplitPlanDto.equal(participants: [me, bob]),
        category: 'food',
        dateMs: 0,
        draft: false,
      ),
    );

    // It counts in the overall net + friend balance, but is in no group.
    expect(await engine.overallNetCents(), 1000);
    expect(await engine.groups(), isEmpty);
    final friend = (await engine.friendDetail(userId: bob))!;
    expect(friend.netCents, 1000);
    expect(friend.shared.single.groupId, isNull);

    // A non-group settlement clears it.
    await engine.recordNonGroupSettlement(from: bob, to: me, amountCents: 1000);
    expect(await engine.overallNetCents(), 0);
  });

  test('draft, publish and closed-period flow through the FFI', () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_bridge_lc');
    addTearDown(() => tmp.deleteSync(recursive: true));

    final engine = await Engine.open(
      dbPath: '${tmp.path}/engine.redb',
      identitySeed: List.filled(32, 5),
      deviceSeed: List.filled(32, 5 + 100),
      dbKey: List.filled(32, 17),
      site: BigInt.from(3),
    );
    await engine.setMyName(name: 'Me');
    final me = await engine.myUserId();
    final bob = await engine.addPerson(name: 'Bob');
    final group = await engine.createGroup(name: 'Trip', memberIds: [bob], currency: 'USD');

    ExpenseInput input({required bool draft, required int dateMs}) => ExpenseInput(
          groupId: group,
          description: 'Dinner',
          paidBy: [Payer(userId: me, cents: 2000)],
          totalCents: 2000,
          split: SplitPlanDto.equal(participants: [me, bob]),
          category: 'food',
          dateMs: dateMs,
          draft: draft,
        );

    // A draft is in the ledger but does not move balances.
    final expense = await engine.addExpense(input: input(draft: true, dateMs: 100));
    var detail = (await engine.groupDetail(groupId: group))!;
    expect(detail.expenses.single.published, isFalse);
    expect(detail.settleUp, isEmpty);

    // Publishing makes it count.
    await engine.publishExpense(expenseId: expense);
    detail = (await engine.groupDetail(groupId: group))!;
    expect(detail.expenses.single.published, isTrue);
    expect(detail.settleUp, hasLength(1));

    // Closing the period freezes edits; reopening unfreezes them.
    await engine.setClosedPeriod(groupId: group, untilMs: 200);
    await expectLater(
      engine.editExpense(expenseId: expense, input: input(draft: false, dateMs: 100)),
      throwsA(anything),
    );
    await engine.setClosedPeriod(groupId: group, untilMs: 0);
    await engine.editExpense(
      expenseId: expense,
      input: input(draft: false, dateMs: 100),
    );
  });

  test('device enrolment + recovery phrase through the FFI (#16/#34)', () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_bridge_dev');
    addTearDown(() => tmp.deleteSync(recursive: true));

    final idSeed = List.filled(32, 61);
    final engine = await Engine.open(
      dbPath: '${tmp.path}/engine.redb',
      identitySeed: idSeed,
      deviceSeed: List.filled(32, 62),
      dbKey: List.filled(32, 63),
      site: BigInt.from(8),
    );
    await engine.setMyName(name: 'Me');

    // This device is self-authorized on open.
    var devices = await engine.listDevices();
    expect(devices.single.thisDevice, isTrue);
    expect(devices.single.device, await engine.myDevicePublic());

    // Enrol + revoke a second device.
    final second = '22' * 32;
    await engine.authorizeDevice(deviceHex: second, site: BigInt.from(9));
    devices = await engine.listDevices();
    expect(devices.any((d) => d.device == second && !d.revoked), isTrue);
    await engine.revokeDevice(deviceHex: second);
    devices = await engine.listDevices();
    expect(devices.firstWhere((d) => d.device == second).revoked, isTrue);

    // The recovery phrase round-trips to the same seed.
    final phrase = await recoveryPhrase(identitySeed: idSeed);
    expect(phrase.split(' ').length, 24);
    expect(await seedFromRecoveryPhrase(phrase: phrase), idSeed);
  });

  test('root lock/unlock gates privileged actions through the FFI (#34)',
      () async {
    // The device-only *reopen* path is covered by the Rust FFI test (it needs
    // the first engine dropped to release the redb lock, which Dart can't force
    // in-process); here we exercise the lock/unlock bindings the shell calls.
    final tmp = Directory.systemTemp.createTempSync('splittr_bridge_do');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final idSeed = List.filled(32, 71);
    final engine = await Engine.open(
      dbPath: '${tmp.path}/engine.redb',
      identitySeed: idSeed,
      deviceSeed: List.filled(32, 72),
      dbKey: List.filled(32, 73),
      site: BigInt.from(10),
    );
    await engine.setMyName(name: 'Me');

    // The identity public key (persisted by the shell for device-only launches).
    expect((await engine.identityPublic()).length, 64);

    // Sealing the root gates privileged actions; daily ops keep working.
    await engine.lockRoot();
    expect(await engine.isRootUnlocked(), isFalse);
    await engine.addPerson(name: 'Bob');
    final second = '33' * 32;
    await expectLater(
      engine.authorizeDevice(deviceHex: second, site: BigInt.from(11)),
      throwsA(anything),
    );

    // Unlocking from the identity seed re-enables them; re-sealing gates again.
    await engine.unlockRoot(identitySeed: idSeed);
    expect(await engine.isRootUnlocked(), isTrue);
    await engine.authorizeDevice(deviceHex: second, site: BigInt.from(11));
    expect((await engine.listDevices()).any((d) => d.device == second), isTrue);
    await engine.lockRoot();
    expect(await engine.isRootUnlocked(), isFalse);
  });

  test('merging duplicate people preserves balances (#8)', () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_bridge_merge');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final engine = await Engine.open(
      dbPath: '${tmp.path}/engine.redb',
      identitySeed: List.filled(32, 91),
      deviceSeed: List.filled(32, 92),
      dbKey: List.filled(32, 93),
      site: BigInt.from(12),
    );
    await engine.setMyName(name: 'Me');
    final me = await engine.myUserId();
    final bob = await engine.addPerson(name: 'Bob');
    final group =
        await engine.createGroup(name: 'Trip', memberIds: [bob], currency: 'USD');
    await engine.addExpense(
      input: ExpenseInput(
        groupId: group,
        description: 'Hotel',
        paidBy: [Payer(userId: me, cents: 3000)],
        totalCents: 3000,
        split: SplitPlanDto.equal(participants: [me, bob]),
        category: 'travel',
        dateMs: 0,
        draft: false,
      ),
    );
    expect(await engine.overallNetCents(), 1500);

    // A duplicate guest for the same person, merged into Bob: my balance is
    // unchanged and the two ids collapse to one friend balance.
    final dup = await engine.addPerson(name: 'Bobby');
    await engine.mergePeople(duplicate: dup, keep: bob);
    expect(await engine.overallNetCents(), 1500);
    final withBalance = (await engine.friends())
        .where((f) => (f.userId == bob || f.userId == dup) && f.netCents != 0)
        .length;
    expect(withBalance, 1);
  });

  test('friend invites + declarations through the FFI (#7)', () async {
    final tmp = Directory.systemTemp.createTempSync('splittr_bridge_inv');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final engine = await Engine.open(
      dbPath: '${tmp.path}/engine.redb',
      identitySeed: List.filled(32, 101),
      deviceSeed: List.filled(32, 102),
      dbKey: List.filled(32, 103),
      site: BigInt.from(14),
    );
    await engine.setMyName(name: 'Me');

    // A friend invite verifies and previews; expiry invalidates it (#7).
    final token = await engine.createFriendInvite(expiryMs: BigInt.from(1000));
    final dto = (await verifyInvite(invite: token, nowMs: BigInt.from(500)))!;
    expect(dto.context, 'friend');
    expect(dto.valid, isTrue);
    expect(dto.inviter, 'id:${await engine.identityPublic()}');
    final expired =
        (await verifyInvite(invite: token, nowMs: BigInt.from(2000)))!;
    expect(expired.valid, isFalse);

    // Tampering breaks the signature (or fails to decode entirely).
    final bad = List<int>.from(token)..last ^= 0xff;
    final tampered = await verifyInvite(invite: bad, nowMs: BigInt.from(500));
    expect(tampered == null || !tampered.valid, isTrue);

    // A one-sided declaration is not yet a confirmed friendship.
    await engine.addFriend(userId: 'id:${'11' * 32}');
    expect(await engine.confirmedFriends(), isEmpty);

    // A group invite carries the group id as its context.
    final group =
        await engine.createGroup(name: 'Trip', memberIds: [], currency: 'USD');
    final gtoken =
        await engine.createGroupInvite(groupId: group, expiryMs: BigInt.from(1000));
    final gdto = (await verifyInvite(invite: gtoken, nowMs: BigInt.from(500)))!;
    expect(gdto.context, 'group:$group');
    expect(gdto.valid, isTrue);
  });

  test('root seed vault + pairing SAS through the FFI (#34/#35)', () async {
    final seed = List.filled(32, 80);
    final blob = await sealRootSeed(passphrase: '1234', seed: seed);
    expect(blob, isNot(equals(seed)));
    expect(await openRootSeed(passphrase: '1234', blob: blob), seed);
    expect(await openRootSeed(passphrase: '0000', blob: blob), isNull);

    final sas = await pairingShortAuthString(
      identityHex: 'aa' * 32,
      primaryDeviceHex: 'bb' * 32,
      newDeviceHex: 'cc' * 32,
      challenge: List.filled(32, 7),
    );
    expect(sas.replaceAll(' ', '').length, 6);
    final tampered = await pairingShortAuthString(
      identityHex: 'aa' * 32,
      primaryDeviceHex: 'bb' * 32,
      newDeviceHex: 'dd' * 32,
      challenge: List.filled(32, 7),
    );
    expect(sas, isNot(equals(tampered)));
  });
}
