/// Core enumerations used across the domain layer.
///
/// Keeping these in one place avoids circular imports between models and makes
/// it trivial to extend behaviour (e.g. adding a new split strategy) from a
/// single, well-known location.
library;

/// The strategy used to divide an expense between participants.
enum SplitType {
  /// Everyone owes an equal share (remainder cents distributed fairly).
  equal,

  /// Each participant owes an explicit amount entered by the user.
  exact,

  /// Each participant owes a percentage of the total (percentages sum to 100).
  percentage,

  /// Each participant owes a portion proportional to a number of shares.
  shares,
}

extension SplitTypeX on SplitType {
  String get label => switch (this) {
        SplitType.equal => 'Equally',
        SplitType.exact => 'Exact amounts',
        SplitType.percentage => 'Percentages',
        SplitType.shares => 'Shares',
      };

  String get storageKey => name;

  static SplitType fromStorage(String value) =>
      SplitType.values.firstWhere((e) => e.name == value,
          orElse: () => SplitType.equal);
}

/// The kind of event recorded in the activity feed.
enum ActivityType {
  expenseAdded,
  expenseUpdated,
  expenseDeleted,
  settlement,
  groupCreated,
  memberAdded,
}

extension ActivityTypeX on ActivityType {
  String get storageKey => name;

  static ActivityType fromStorage(String value) =>
      ActivityType.values.firstWhere((e) => e.name == value,
          orElse: () => ActivityType.expenseAdded);
}
