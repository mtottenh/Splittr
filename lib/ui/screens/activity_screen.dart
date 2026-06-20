import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';

import '../../domain/models/activity.dart';
import '../../domain/models/enums.dart';
import '../../state/providers.dart';
import '../widgets/empty_state.dart';

/// Reverse-chronological feed of everything that has happened.
class ActivityScreen extends ConsumerWidget {
  const ActivityScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final activities = ref.watch(appControllerProvider).activities;

    return Scaffold(
      appBar: AppBar(title: const Text('Activity')),
      body: activities.isEmpty
          ? const EmptyState(
              icon: Icons.history,
              title: 'No activity yet',
              message: 'Your expenses and payments will show up here.',
            )
          : ListView.separated(
              itemCount: activities.length,
              separatorBuilder: (_, _) => const Divider(height: 1),
              itemBuilder: (context, i) => _ActivityTile(activities[i]),
            ),
    );
  }
}

class _ActivityTile extends StatelessWidget {
  const _ActivityTile(this.activity);
  final Activity activity;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return ListTile(
      leading: CircleAvatar(
        backgroundColor: theme.colorScheme.surfaceContainerHighest,
        child: Icon(_iconFor(activity.type), size: 20),
      ),
      title: Text(activity.summary),
      subtitle: Text(_relativeTime(activity.timestamp)),
    );
  }

  IconData _iconFor(ActivityType type) => switch (type) {
        ActivityType.expenseAdded => Icons.add_circle_outline,
        ActivityType.expenseUpdated => Icons.edit_outlined,
        ActivityType.expenseDeleted => Icons.delete_outline,
        ActivityType.settlement => Icons.swap_horiz,
        ActivityType.groupCreated => Icons.group_add,
        ActivityType.memberAdded => Icons.person_add,
      };

  String _relativeTime(DateTime time) {
    final diff = DateTime.now().difference(time);
    if (diff.inMinutes < 1) return 'just now';
    if (diff.inMinutes < 60) return '${diff.inMinutes}m ago';
    if (diff.inHours < 24) return '${diff.inHours}h ago';
    if (diff.inDays < 7) return '${diff.inDays}d ago';
    return DateFormat.yMMMd().format(time);
  }
}
