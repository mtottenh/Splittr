import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/app_repository.dart';
import '../data/persistence.dart';
import '../domain/models/balance.dart';
import '../domain/services/balance_calculator.dart';
import 'app_controller.dart';
import 'app_state.dart';

/// The storage backend. Native platforms persist to a JSON file; the web has
/// no documents directory, so it falls back to in-memory storage. Overridden in
/// tests with [InMemoryPersistence].
final persistenceProvider = Provider<Persistence>(
  (ref) => kIsWeb ? InMemoryPersistence() : FilePersistence(),
);

final repositoryProvider = Provider<AppRepository>(
  (ref) => AppRepository(ref.watch(persistenceProvider)),
);

/// The app's single source of truth.
final appControllerProvider =
    StateNotifierProvider<AppController, AppState>((ref) {
  return AppController(ref.watch(repositoryProvider));
});

/// Runs one-time startup: loads persisted data or seeds a fresh profile. The
/// UI watches this to show a splash until the state is ready.
final bootstrapProvider = FutureProvider<void>((ref) async {
  await ref.read(appControllerProvider.notifier).bootstrap();
});

/// Net balances across the *entire* ledger (used for friend totals and the
/// top-level "you are owed / you owe" summary).
final overallNetBalancesProvider = Provider<Map<String, int>>((ref) {
  final state = ref.watch(appControllerProvider);
  return BalanceCalculator.netBalances(
    expenses: state.expenses,
    settlements: state.settlements,
  );
});

/// Pairwise debts across the entire ledger, for friend-to-friend balances.
final overallPairwiseProvider = Provider<List<DebtEdge>>((ref) {
  final state = ref.watch(appControllerProvider);
  return BalanceCalculator.pairwiseDebts(
    expenses: state.expenses,
    settlements: state.settlements,
  );
});

/// The current user's overall net position in cents (positive = owed to you).
final currentUserNetProvider = Provider<int>((ref) {
  final state = ref.watch(appControllerProvider);
  final net = ref.watch(overallNetBalancesProvider);
  return net[state.currentUserId] ?? 0;
});

/// Net balances within a single group, keyed by group id.
final groupNetBalancesProvider =
    Provider.family<Map<String, int>, String>((ref, groupId) {
  final state = ref.watch(appControllerProvider);
  return BalanceCalculator.netBalances(
    expenses: state.expensesForGroup(groupId),
    settlements: state.settlementsForGroup(groupId),
  );
});

/// Suggested settle-up payments within a group, honouring its simplify setting.
final groupSettleUpProvider =
    Provider.family<List<DebtEdge>, String>((ref, groupId) {
  final state = ref.watch(appControllerProvider);
  final group = state.groupById(groupId);
  return BalanceCalculator.settleUpSuggestions(
    expenses: state.expensesForGroup(groupId),
    settlements: state.settlementsForGroup(groupId),
    simplify: group?.simplifyDebts ?? true,
  );
});
