import '../src/rust/dto.dart';

/// The top-level snapshot the shell renders: the local user, their groups,
/// friends and the activity feed. Assembled from the engine's query methods and
/// rebuilt after every mutation. Per-group and per-friend detail are loaded
/// lazily by their own providers.
class AppData {
  const AppData({
    required this.myUserId,
    required this.myName,
    required this.overallNetCents,
    required this.groups,
    required this.friends,
    required this.activity,
  });

  final String myUserId;
  final String myName;

  /// The user's overall net across all expenses/settlements, including
  /// non-group ones (positive = owed to you). Computed by the engine (#31).
  final int overallNetCents;

  final List<GroupSummaryDto> groups;
  final List<FriendBalanceDto> friends;
  final List<ActivityEntryDto> activity;
}
