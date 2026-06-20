import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:uuid/uuid.dart';

import '../core/money.dart';
import '../data/app_repository.dart';
import '../domain/models/activity.dart';
import '../domain/models/enums.dart';
import '../domain/models/expense.dart';
import '../domain/models/group.dart';
import '../domain/models/settlement.dart';
import '../domain/models/split.dart';
import '../domain/models/user.dart';
import 'app_state.dart';

/// Palette used to give each new profile a distinct avatar colour.
const _avatarColors = <int>[
  0xFF1CC29F, // teal (brand)
  0xFFEF6C57,
  0xFF5B8DEF,
  0xFFF2A93B,
  0xFF9B6CD6,
  0xFF3FB68B,
  0xFFE05A8E,
  0xFF4C9AA8,
];

/// Owns the single source of truth and exposes every mutation the UI needs.
///
/// Each mutation produces a new immutable [AppState], pushes it to listeners,
/// and persists it. Business invariants (split sums, payer sums) are enforced
/// here so no inconsistent state can ever be stored.
class AppController extends StateNotifier<AppState> {
  AppController(this._repository, {Uuid? uuid})
      : _uuid = uuid ?? const Uuid(),
        super(AppState.empty);

  final AppRepository _repository;
  final Uuid _uuid;

  bool _loaded = false;
  bool get isLoaded => _loaded;

  /// Loads persisted state, seeding a fresh "you" profile on first launch.
  Future<void> bootstrap() async {
    final loaded = await _repository.load();
    if (loaded != null && loaded.currentUserId.isNotEmpty) {
      state = loaded;
    } else {
      final me = AppUser(
        id: _uuid.v4(),
        name: 'You',
        avatarColor: _avatarColors.first,
      );
      state = AppState.empty
          .copyWith(users: [me], currentUserId: me.id);
      await _persist();
    }
    _loaded = true;
  }

  Future<void> _persist() => _repository.save(state);

  // --- Profiles & people -------------------------------------------------

  Future<void> updateCurrentUser({required String name, String? email}) async {
    final updated = [
      for (final u in state.users)
        if (u.id == state.currentUserId)
          u.copyWith(name: name, email: email)
        else
          u,
    ];
    state = state.copyWith(users: updated);
    await _persist();
  }

  /// Adds a new person (a friend or group member) and returns them.
  Future<AppUser> addPerson({required String name, String? email}) async {
    final user = AppUser(
      id: _uuid.v4(),
      name: name.trim(),
      email: email?.trim().isEmpty ?? true ? null : email!.trim(),
      avatarColor: _avatarColors[state.users.length % _avatarColors.length],
    );
    state = state.copyWith(users: [...state.users, user]);
    await _persist();
    return user;
  }

  // --- Groups ------------------------------------------------------------

  Future<Group> createGroup({
    required String name,
    required List<String> memberIds,
    String currencyCode = 'USD',
    String emoji = '🧾',
    bool simplifyDebts = true,
  }) async {
    final members = {state.currentUserId, ...memberIds}.toList();
    final group = Group(
      id: _uuid.v4(),
      name: name.trim(),
      memberIds: members,
      currencyCode: currencyCode,
      emoji: emoji,
      simplifyDebts: simplifyDebts,
      createdAt: DateTime.now(),
    );
    state = state.copyWith(groups: [...state.groups, group]);
    await _log(ActivityType.groupCreated,
        summary: 'You created the group "${group.name}"', groupId: group.id);
    await _persist();
    return group;
  }

  Future<void> updateGroup(Group group) async {
    state = state.copyWith(groups: [
      for (final g in state.groups)
        if (g.id == group.id) group else g,
    ]);
    await _persist();
  }

  Future<void> addMemberToGroup(String groupId, String userId) async {
    final group = state.groupById(groupId);
    if (group == null || group.memberIds.contains(userId)) return;
    final updated = group.copyWith(memberIds: [...group.memberIds, userId]);
    state = state.copyWith(groups: [
      for (final g in state.groups)
        if (g.id == groupId) updated else g,
    ]);
    await _log(ActivityType.memberAdded,
        summary: '${state.displayName(userId)} was added to "${group.name}"',
        groupId: groupId);
    await _persist();
  }

  /// Deletes a group along with its expenses and settlements.
  Future<void> deleteGroup(String groupId) async {
    state = state.copyWith(
      groups: state.groups.where((g) => g.id != groupId).toList(),
      expenses: state.expenses.where((e) => e.groupId != groupId).toList(),
      settlements:
          state.settlements.where((s) => s.groupId != groupId).toList(),
      activities:
          state.activities.where((a) => a.groupId != groupId).toList(),
    );
    await _persist();
  }

  // --- Expenses ----------------------------------------------------------

  Future<Expense> addExpense({
    required String? groupId,
    required String description,
    required int totalCents,
    required String currencyCode,
    required Map<String, int> paidBy,
    required List<Split> splits,
    required SplitType splitType,
    required String categoryId,
    DateTime? date,
    String? notes,
  }) async {
    _validateSums(totalCents: totalCents, paidBy: paidBy, splits: splits);
    final expense = Expense(
      id: _uuid.v4(),
      groupId: groupId,
      description: description.trim(),
      totalCents: totalCents,
      currencyCode: currencyCode,
      paidBy: paidBy,
      splits: splits,
      splitType: splitType,
      categoryId: categoryId,
      date: date ?? DateTime.now(),
      createdAt: DateTime.now(),
      notes: notes?.trim().isEmpty ?? true ? null : notes!.trim(),
    );
    state = state.copyWith(expenses: [...state.expenses, expense]);
    await _log(
      ActivityType.expenseAdded,
      summary:
          'You added "${expense.description}" (${Money.format(totalCents, code: currencyCode)})',
      groupId: groupId,
      expenseId: expense.id,
      amountCents: totalCents,
      currencyCode: currencyCode,
    );
    await _persist();
    return expense;
  }

  Future<void> updateExpense(Expense expense) async {
    _validateSums(
      totalCents: expense.totalCents,
      paidBy: expense.paidBy,
      splits: expense.splits,
    );
    state = state.copyWith(expenses: [
      for (final e in state.expenses)
        if (e.id == expense.id) expense else e,
    ]);
    await _log(
      ActivityType.expenseUpdated,
      summary: 'You updated "${expense.description}"',
      groupId: expense.groupId,
      expenseId: expense.id,
    );
    await _persist();
  }

  Future<void> deleteExpense(String expenseId) async {
    Expense? expense;
    for (final e in state.expenses) {
      if (e.id == expenseId) {
        expense = e;
        break;
      }
    }
    state = state.copyWith(
      expenses: state.expenses.where((e) => e.id != expenseId).toList(),
    );
    if (expense != null) {
      await _log(
        ActivityType.expenseDeleted,
        summary: 'You deleted "${expense.description}"',
        groupId: expense.groupId,
      );
    }
    await _persist();
  }

  // --- Settlements -------------------------------------------------------

  Future<Settlement> addSettlement({
    required String? groupId,
    required String fromUserId,
    required String toUserId,
    required int amountCents,
    required String currencyCode,
    DateTime? date,
    String? note,
  }) async {
    final settlement = Settlement(
      id: _uuid.v4(),
      groupId: groupId,
      fromUserId: fromUserId,
      toUserId: toUserId,
      amountCents: amountCents,
      currencyCode: currencyCode,
      date: date ?? DateTime.now(),
      note: note,
    );
    state = state.copyWith(settlements: [...state.settlements, settlement]);
    await _log(
      ActivityType.settlement,
      summary:
          '${state.displayName(fromUserId)} paid ${state.displayName(toUserId)} '
          '${Money.format(amountCents, code: currencyCode)}',
      groupId: groupId,
      amountCents: amountCents,
      currencyCode: currencyCode,
    );
    await _persist();
    return settlement;
  }

  Future<void> deleteSettlement(String settlementId) async {
    state = state.copyWith(
      settlements:
          state.settlements.where((s) => s.id != settlementId).toList(),
    );
    await _persist();
  }

  // --- Internals ---------------------------------------------------------

  void _validateSums({
    required int totalCents,
    required Map<String, int> paidBy,
    required List<Split> splits,
  }) {
    final paid = paidBy.values.fold(0, (a, b) => a + b);
    final owed = splits.fold(0, (a, s) => a + s.owedCents);
    assert(
      paid == totalCents,
      'Payments ($paid) must equal the total ($totalCents).',
    );
    assert(
      owed == totalCents,
      'Splits ($owed) must equal the total ($totalCents).',
    );
    if (paid != totalCents || owed != totalCents) {
      throw StateError('Expense does not balance.');
    }
  }

  Future<void> _log(
    ActivityType type, {
    required String summary,
    String? groupId,
    String? expenseId,
    int? amountCents,
    String? currencyCode,
  }) async {
    final activity = Activity(
      id: _uuid.v4(),
      type: type,
      actorId: state.currentUserId,
      summary: summary,
      timestamp: DateTime.now(),
      groupId: groupId,
      expenseId: expenseId,
      amountCents: amountCents,
      currencyCode: currencyCode,
    );
    // Newest first, capped so the feed never grows unbounded.
    final activities = [activity, ...state.activities];
    if (activities.length > 500) activities.removeRange(500, activities.length);
    state = state.copyWith(activities: activities);
  }
}
