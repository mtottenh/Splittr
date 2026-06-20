import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/data/persistence.dart';
import 'package:splittr/domain/models/enums.dart';
import 'package:splittr/domain/models/split.dart';
import 'package:splittr/domain/services/split_calculator.dart';
import 'package:splittr/state/providers.dart';

void main() {
  late ProviderContainer container;
  late InMemoryPersistence persistence;

  setUp(() {
    persistence = InMemoryPersistence();
    container = ProviderContainer(overrides: [
      persistenceProvider.overrideWithValue(persistence),
    ]);
  });

  tearDown(() => container.dispose());

  test('bootstrap seeds a current user on first launch', () async {
    await container.read(appControllerProvider.notifier).bootstrap();
    final state = container.read(appControllerProvider);
    expect(state.users, hasLength(1));
    expect(state.currentUserId, isNotEmpty);
  });

  test('full flow: group, expense, balance and settlement', () async {
    final controller = container.read(appControllerProvider.notifier);
    await controller.bootstrap();
    final me = container.read(appControllerProvider).currentUserId;

    final alice = await controller.addPerson(name: 'Alice');
    final group = await controller.createGroup(
      name: 'Trip',
      memberIds: [alice.id],
    );

    const total = 4000;
    await controller.addExpense(
      groupId: group.id,
      description: 'Hotel',
      totalCents: total,
      currencyCode: 'USD',
      paidBy: {me: total},
      splits: SplitCalculator.equal(totalCents: total, userIds: [me, alice.id]),
      splitType: SplitType.equal,
      categoryId: 'travel',
    );

    final net = container.read(groupNetBalancesProvider(group.id));
    expect(net[me], 2000); // paid 40, owes 20 -> owed 20
    expect(net[alice.id], -2000);

    final suggestions = container.read(groupSettleUpProvider(group.id));
    expect(suggestions, hasLength(1));
    expect(suggestions.single.fromUserId, alice.id);
    expect(suggestions.single.toUserId, me);
    expect(suggestions.single.amountCents, 2000);

    // Alice settles up; balances clear.
    await controller.addSettlement(
      groupId: group.id,
      fromUserId: alice.id,
      toUserId: me,
      amountCents: 2000,
      currencyCode: 'USD',
    );
    expect(container.read(groupNetBalancesProvider(group.id)), isEmpty);

    // Activity feed recorded the events.
    expect(container.read(appControllerProvider).activities, isNotEmpty);
  });

  test('state persists and reloads across controller instances', () async {
    final controller = container.read(appControllerProvider.notifier);
    await controller.bootstrap();
    await controller.addPerson(name: 'Bob');

    // New container sharing the same persistence backend.
    final reopened = ProviderContainer(overrides: [
      persistenceProvider.overrideWithValue(persistence),
    ]);
    addTearDown(reopened.dispose);
    await reopened.read(appControllerProvider.notifier).bootstrap();

    final names =
        reopened.read(appControllerProvider).users.map((u) => u.name);
    expect(names, contains('Bob'));
  });

  test('rejects an expense whose splits do not equal the total', () async {
    final controller = container.read(appControllerProvider.notifier);
    await controller.bootstrap();
    final me = container.read(appControllerProvider).currentUserId;

    expect(
      () => controller.addExpense(
        groupId: null,
        description: 'Bad',
        totalCents: 1000,
        currencyCode: 'USD',
        paidBy: {me: 1000},
        // Splits sum to 999, not 1000.
        splits: const [Split(userId: 'x', owedCents: 999)],
        splitType: SplitType.exact,
        categoryId: 'general',
      ),
      throwsA(isA<Error>()),
    );
  });
}
