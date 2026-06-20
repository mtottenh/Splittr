import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/money.dart';
import '../../core/theme.dart';
import '../../src/rust/dto.dart';
import '../../state/providers.dart';
import '../widgets/balance_label.dart';
import '../widgets/empty_state.dart';
import 'create_group_screen.dart';
import 'group_detail_screen.dart';

/// Lists the user's groups with a per-group balance, topped by an overall
/// "you owe / are owed" summary.
class GroupsScreen extends ConsumerWidget {
  const GroupsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final data = ref.watch(appProvider).requireValue;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Groups'),
        actions: [
          IconButton(
            tooltip: 'Create group',
            icon: const Icon(Icons.group_add),
            onPressed: () => _createGroup(context),
          ),
        ],
      ),
      body: data.groups.isEmpty
          ? EmptyState(
              icon: Icons.groups,
              title: 'No groups yet',
              message:
                  'Create a group for a trip, household or any shared expenses.',
              action: FilledButton.icon(
                onPressed: () => _createGroup(context),
                icon: const Icon(Icons.add),
                label: const Text('Create a group'),
              ),
            )
          : ListView(
              padding: const EdgeInsets.only(bottom: 96),
              children: [
                _OverallSummary(netCents: data.overallNetCents),
                for (final group in data.groups) _GroupTile(group: group),
              ],
            ),
    );
  }

  Future<void> _createGroup(BuildContext context) {
    return Navigator.of(context).push(
      MaterialPageRoute<void>(builder: (_) => const CreateGroupScreen()),
    );
  }
}

class _OverallSummary extends StatelessWidget {
  const _OverallSummary({required this.netCents});
  final int netCents;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final color = AppTheme.balanceColor(netCents, theme.colorScheme);
    final String text;
    if (netCents > 0) {
      text = 'Overall, you are owed ${Money.formatAbs(netCents)}';
    } else if (netCents < 0) {
      text = 'Overall, you owe ${Money.formatAbs(netCents)}';
    } else {
      text = "You're all settled up";
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
      child: Text(
        text,
        style: theme.textTheme.titleMedium
            ?.copyWith(color: color, fontWeight: FontWeight.w600),
      ),
    );
  }
}

class _GroupTile extends StatelessWidget {
  const _GroupTile({required this.group});
  final GroupSummaryDto group;

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.fromLTRB(12, 6, 12, 6),
      child: ListTile(
        leading: CircleAvatar(
          backgroundColor: Theme.of(context).colorScheme.primaryContainer,
          child: const Icon(Icons.groups),
        ),
        title: Text(group.name,
            style: const TextStyle(fontWeight: FontWeight.w600)),
        subtitle: Text(
          '${group.memberCount} '
          '${group.memberCount == 1 ? "member" : "members"}',
        ),
        trailing:
            BalanceLabel(netCents: group.myNetCents, currencyCode: group.currency),
        onTap: () => Navigator.of(context).push(
          MaterialPageRoute<void>(
            builder: (_) => GroupDetailScreen(groupId: group.id),
          ),
        ),
      ),
    );
  }
}
