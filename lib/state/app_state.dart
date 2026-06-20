import 'package:flutter/foundation.dart';

import '../domain/models/activity.dart';
import '../domain/models/expense.dart';
import '../domain/models/group.dart';
import '../domain/models/settlement.dart';
import '../domain/models/user.dart';

/// The complete, immutable snapshot of everything the app knows.
///
/// Treating state as one immutable value makes the controller's mutations
/// explicit (every change returns a new snapshot) and makes serialization a
/// single [toJson] call.
@immutable
class AppState {
  const AppState({
    required this.users,
    required this.groups,
    required this.expenses,
    required this.settlements,
    required this.activities,
    required this.currentUserId,
  });

  final List<AppUser> users;
  final List<Group> groups;
  final List<Expense> expenses;
  final List<Settlement> settlements;
  final List<Activity> activities;

  /// The id of the profile using this device ("you").
  final String currentUserId;

  static const empty = AppState(
    users: [],
    groups: [],
    expenses: [],
    settlements: [],
    activities: [],
    currentUserId: '',
  );

  AppUser get currentUser => users.firstWhere(
        (u) => u.id == currentUserId,
        orElse: () => const AppUser(id: '', name: 'You'),
      );

  AppUser? userById(String id) {
    for (final u in users) {
      if (u.id == id) return u;
    }
    return null;
  }

  String displayName(String userId) {
    if (userId == currentUserId) return 'You';
    return userById(userId)?.name ?? 'Unknown';
  }

  Group? groupById(String id) {
    for (final g in groups) {
      if (g.id == id) return g;
    }
    return null;
  }

  List<Expense> expensesForGroup(String groupId) =>
      expenses.where((e) => e.groupId == groupId).toList()
        ..sort((a, b) => b.date.compareTo(a.date));

  List<Settlement> settlementsForGroup(String groupId) =>
      settlements.where((s) => s.groupId == groupId).toList()
        ..sort((a, b) => b.date.compareTo(a.date));

  AppState copyWith({
    List<AppUser>? users,
    List<Group>? groups,
    List<Expense>? expenses,
    List<Settlement>? settlements,
    List<Activity>? activities,
    String? currentUserId,
  }) =>
      AppState(
        users: users ?? this.users,
        groups: groups ?? this.groups,
        expenses: expenses ?? this.expenses,
        settlements: settlements ?? this.settlements,
        activities: activities ?? this.activities,
        currentUserId: currentUserId ?? this.currentUserId,
      );

  Map<String, dynamic> toJson() => {
        'version': 1,
        'currentUserId': currentUserId,
        'users': users.map((e) => e.toJson()).toList(),
        'groups': groups.map((e) => e.toJson()).toList(),
        'expenses': expenses.map((e) => e.toJson()).toList(),
        'settlements': settlements.map((e) => e.toJson()).toList(),
        'activities': activities.map((e) => e.toJson()).toList(),
      };

  factory AppState.fromJson(Map<String, dynamic> json) => AppState(
        currentUserId: json['currentUserId'] as String? ?? '',
        users: (json['users'] as List<dynamic>? ?? [])
            .map((e) => AppUser.fromJson(e as Map<String, dynamic>))
            .toList(),
        groups: (json['groups'] as List<dynamic>? ?? [])
            .map((e) => Group.fromJson(e as Map<String, dynamic>))
            .toList(),
        expenses: (json['expenses'] as List<dynamic>? ?? [])
            .map((e) => Expense.fromJson(e as Map<String, dynamic>))
            .toList(),
        settlements: (json['settlements'] as List<dynamic>? ?? [])
            .map((e) => Settlement.fromJson(e as Map<String, dynamic>))
            .toList(),
        activities: (json['activities'] as List<dynamic>? ?? [])
            .map((e) => Activity.fromJson(e as Map<String, dynamic>))
            .toList(),
      );
}
