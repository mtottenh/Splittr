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
}
