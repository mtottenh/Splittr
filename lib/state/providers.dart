import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../src/rust/api.dart';
import '../src/rust/dto.dart';
import 'app_data.dart';
import 'engine.dart';

/// The app's single source of truth: an engine-backed snapshot plus every
/// mutation the UI needs. Mutations call the Rust engine, then refresh the
/// snapshot so all watchers update. Business logic lives in Rust — this is a
/// thin async facade.
final appProvider = AsyncNotifierProvider<AppNotifier, AppData>(AppNotifier.new);

class AppNotifier extends AsyncNotifier<AppData> {
  @override
  Future<AppData> build() async {
    final engine = await ref.watch(engineProvider.future);
    return _snapshot(engine);
  }

  Future<AppData> _snapshot(Engine engine) async {
    var name = await engine.myName();
    if (name == null) {
      // First launch: seed a profile so the local user has a name.
      await engine.setMyName(name: 'You');
      name = 'You';
    }
    return AppData(
      myUserId: await engine.myUserId(),
      myName: name,
      overallNetCents: await engine.overallNetCents(),
      groups: await engine.groups(),
      friends: await engine.friends(),
      activity: await engine.activity(),
    );
  }

  /// Apply [action] to the engine, then rebuild the snapshot in place.
  Future<T> _mutate<T>(Future<T> Function(Engine engine) action) async {
    final engine = await ref.read(engineProvider.future);
    final result = await action(engine);
    state = AsyncData(await _snapshot(engine));
    return result;
  }

  Future<void> setMyName(String name) =>
      _mutate((e) => e.setMyName(name: name));

  Future<String> addPerson(String name) =>
      _mutate((e) => e.addPerson(name: name));

  Future<String> createGroup(String name, List<String> memberIds) =>
      _mutate((e) => e.createGroup(name: name, memberIds: memberIds));

  Future<void> renameGroup(String groupId, String name) =>
      _mutate((e) => e.renameGroup(groupId: groupId, name: name));

  Future<void> addMember(String groupId, String userId) =>
      _mutate((e) => e.addMember(groupId: groupId, userId: userId));

  Future<String> addExpense(ExpenseInput input) =>
      _mutate((e) => e.addExpense(input: input));

  Future<void> editExpense(String expenseId, ExpenseInput input) =>
      _mutate((e) => e.editExpense(expenseId: expenseId, input: input));

  Future<void> deleteExpense(String expenseId) =>
      _mutate((e) => e.deleteExpense(expenseId: expenseId));

  Future<void> publishExpense(String expenseId) =>
      _mutate((e) => e.publishExpense(expenseId: expenseId));

  Future<void> setClosedPeriod(String groupId, int untilMs) =>
      _mutate((e) => e.setClosedPeriod(groupId: groupId, untilMs: untilMs));

  Future<String> recordSettlement({
    required String groupId,
    required String from,
    required String to,
    required int amountCents,
  }) =>
      _mutate((e) => e.recordSettlement(
            groupId: groupId,
            from: from,
            to: to,
            amountCents: amountCents,
          ));

  Future<void> recordNonGroupSettlement({
    required String from,
    required String to,
    required int amountCents,
  }) =>
      _mutate((e) => e.recordNonGroupSettlement(
            from: from,
            to: to,
            amountCents: amountCents,
          ));

  Future<void> deleteSettlement(String settlementId) =>
      _mutate((e) => e.deleteSettlement(settlementId: settlementId));
}

/// Detail for a single group. Re-fetches whenever [appProvider] changes (i.e.
/// after any mutation).
final groupDetailProvider =
    FutureProvider.family<GroupDetailDto?, String>((ref, groupId) async {
  ref.watch(appProvider);
  final engine = await ref.watch(engineProvider.future);
  return engine.groupDetail(groupId: groupId);
});

/// Detail for a single friend, refreshed alongside [appProvider].
final friendDetailProvider =
    FutureProvider.family<FriendDetailDto?, String>((ref, userId) async {
  ref.watch(appProvider);
  final engine = await ref.watch(engineProvider.future);
  return engine.friendDetail(userId: userId);
});
