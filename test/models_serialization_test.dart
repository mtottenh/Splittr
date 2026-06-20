import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/domain/models/enums.dart';
import 'package:splittr/domain/models/expense.dart';
import 'package:splittr/domain/models/group.dart';
import 'package:splittr/domain/models/split.dart';
import 'package:splittr/state/app_state.dart';

void main() {
  test('Expense survives a JSON round-trip', () {
    final expense = Expense(
      id: 'e1',
      groupId: 'g1',
      description: 'Dinner',
      totalCents: 1234,
      currencyCode: 'EUR',
      paidBy: const {'a': 1234},
      splits: const [
        Split(userId: 'a', owedCents: 617),
        Split(userId: 'b', owedCents: 617),
      ],
      splitType: SplitType.percentage,
      categoryId: 'food',
      date: DateTime(2024, 5, 1),
      createdAt: DateTime(2024, 5, 1),
      notes: 'tasty',
    );

    final restored = Expense.fromJson(expense.toJson());
    expect(restored, expense);
    expect(restored.splitType, SplitType.percentage);
    expect(restored.notes, 'tasty');
  });

  test('Group survives a JSON round-trip', () {
    final group = Group(
      id: 'g1',
      name: 'Flat',
      memberIds: const ['a', 'b', 'c'],
      currencyCode: 'GBP',
      emoji: '🏠',
      simplifyDebts: false,
      createdAt: DateTime(2024, 1, 1),
    );
    expect(Group.fromJson(group.toJson()), group);
  });

  test('AppState round-trips through JSON', () {
    final state = AppState.empty.copyWith(
      currentUserId: 'u1',
      groups: [
        Group(
          id: 'g1',
          name: 'Trip',
          memberIds: const ['u1'],
          createdAt: DateTime(2024, 1, 1),
        ),
      ],
    );
    final restored = AppState.fromJson(state.toJson());
    expect(restored.currentUserId, 'u1');
    expect(restored.groups.single.name, 'Trip');
  });
}
