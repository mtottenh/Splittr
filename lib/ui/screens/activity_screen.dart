import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:intl/intl.dart';

import '../../src/rust/dto.dart';
import '../../state/providers.dart';
import '../widgets/empty_state.dart';

/// Reverse-chronological feed derived from the signed op-log.
class ActivityScreen extends ConsumerWidget {
  const ActivityScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final activity = ref.watch(appProvider).requireValue.activity;

    return Scaffold(
      appBar: AppBar(title: const Text('Activity')),
      body: activity.isEmpty
          ? const EmptyState(
              icon: Icons.history,
              title: 'No activity yet',
              message: 'Your expenses and payments will show up here.',
            )
          : ListView.separated(
              itemCount: activity.length,
              separatorBuilder: (_, _) => const Divider(height: 1),
              itemBuilder: (context, i) => _ActivityTile(activity[i]),
            ),
    );
  }
}

class _ActivityTile extends StatelessWidget {
  const _ActivityTile(this.entry);
  final ActivityEntryDto entry;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return ListTile(
      leading: CircleAvatar(
        backgroundColor: theme.colorScheme.surfaceContainerHighest,
        child: Icon(_iconFor(entry.kind), size: 20),
      ),
      title: Text(entry.summary),
      subtitle: Text(_relativeTime(entry.wallMs)),
    );
  }

  IconData _iconFor(String kind) => switch (kind) {
        'expense' => Icons.add_circle_outline,
        'edit' => Icons.edit_outlined,
        'delete' => Icons.delete_outline,
        'settlement' => Icons.swap_horiz,
        'group' => Icons.group_add,
        'person' => Icons.person_add,
        _ => Icons.history,
      };

  String _relativeTime(int wallMs) {
    if (wallMs <= 0) return '';
    final time = DateTime.fromMillisecondsSinceEpoch(wallMs);
    final diff = DateTime.now().difference(time);
    if (diff.inMinutes < 1) return 'just now';
    if (diff.inMinutes < 60) return '${diff.inMinutes}m ago';
    if (diff.inHours < 24) return '${diff.inHours}h ago';
    if (diff.inDays < 7) return '${diff.inDays}d ago';
    return DateFormat.yMMMd().format(time);
  }
}
