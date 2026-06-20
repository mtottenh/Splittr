import '../src/rust/dto.dart';

/// The top-level snapshot the shell renders: the local user, their groups,
/// friends and the activity feed. Assembled from the engine's query methods and
/// rebuilt after every mutation. Per-group and per-friend detail are loaded
/// lazily by their own providers.
class AppData {
  const AppData({
    required this.myUserId,
    required this.myName,
    required this.groups,
    required this.friends,
    required this.activity,
  });

  final String myUserId;
  final String myName;
  final List<GroupSummaryDto> groups;
  final List<FriendBalanceDto> friends;
  final List<ActivityEntryDto> activity;

  /// The user's overall net across all groups (positive = owed to you). Every
  /// expense is group-scoped, so summing the per-group nets gives the total.
  int get overallNetCents =>
      groups.fold(0, (sum, g) => sum + g.myNetCents);
}
