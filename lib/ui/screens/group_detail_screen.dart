import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/categories.dart';
import '../../core/money.dart';
import '../../core/theme.dart';
import '../../domain/models/expense.dart';
import '../../domain/models/settlement.dart';
import '../../state/providers.dart';
import '../widgets/add_person_dialog.dart';
import '../widgets/balance_label.dart';
import '../widgets/empty_state.dart';
import '../widgets/user_avatar.dart';
import 'add_expense_screen.dart';
import 'settle_up_screen.dart';

/// Detailed view of a single group: its ledger and its balances.
class GroupDetailScreen extends ConsumerWidget {
  const GroupDetailScreen({super.key, required this.groupId});

  final String groupId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(appControllerProvider);
    final group = state.groupById(groupId);
    if (group == null) {
      return const Scaffold(body: Center(child: Text('Group not found')));
    }

    return DefaultTabController(
      length: 2,
      child: Scaffold(
        appBar: AppBar(
          title: Row(
            children: [
              Text(group.emoji),
              const SizedBox(width: 8),
              Expanded(child: Text(group.name, overflow: TextOverflow.ellipsis)),
            ],
          ),
          actions: [
            IconButton(
              tooltip: 'Add member',
              icon: const Icon(Icons.person_add_alt),
              onPressed: () => _addMember(context, ref),
            ),
            PopupMenuButton<String>(
              onSelected: (value) {
                if (value == 'delete') _confirmDelete(context, ref);
              },
              itemBuilder: (context) => const [
                PopupMenuItem(value: 'delete', child: Text('Delete group')),
              ],
            ),
          ],
          bottom: const TabBar(
            tabs: [Tab(text: 'Expenses'), Tab(text: 'Balances')],
          ),
        ),
        floatingActionButton: FloatingActionButton.extended(
          onPressed: () => Navigator.of(context).push(
            MaterialPageRoute<void>(
              builder: (_) => AddExpenseScreen(groupId: groupId),
              fullscreenDialog: true,
            ),
          ),
          icon: const Icon(Icons.add),
          label: const Text('Add expense'),
        ),
        body: TabBarView(
          children: [
            _ExpensesTab(groupId: groupId),
            _BalancesTab(groupId: groupId),
          ],
        ),
      ),
    );
  }

  Future<void> _addMember(BuildContext context, WidgetRef ref) async {
    final state = ref.read(appControllerProvider);
    final group = state.groupById(groupId)!;
    final candidates = state.users
        .where((u) => !group.memberIds.contains(u.id))
        .toList();

    final selected = await showModalBottomSheet<String>(
      context: context,
      builder: (context) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: [
            ListTile(
              leading: const Icon(Icons.person_add),
              title: const Text('Add a new person'),
              onTap: () async {
                final user = await showAddPersonDialog(context, ref);
                if (context.mounted) Navigator.of(context).pop(user?.id);
              },
            ),
            const Divider(),
            for (final u in candidates)
              ListTile(
                leading: UserAvatar(user: u),
                title: Text(u.name),
                onTap: () => Navigator.of(context).pop(u.id),
              ),
          ],
        ),
      ),
    );
    if (selected != null) {
      await ref
          .read(appControllerProvider.notifier)
          .addMemberToGroup(groupId, selected);
    }
  }

  Future<void> _confirmDelete(BuildContext context, WidgetRef ref) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Delete group?'),
        content: const Text(
            'This permanently removes the group and all of its expenses.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Delete'),
          ),
        ],
      ),
    );
    if (confirmed ?? false) {
      await ref.read(appControllerProvider.notifier).deleteGroup(groupId);
      if (context.mounted) Navigator.of(context).pop();
    }
  }
}

/// A ledger entry can be an expense or a settlement; this unifies them for the
/// chronological list.
class _ExpensesTab extends ConsumerWidget {
  const _ExpensesTab({required this.groupId});
  final String groupId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(appControllerProvider);
    final group = state.groupById(groupId)!;
    final expenses = state.expensesForGroup(groupId);
    final settlements = state.settlementsForGroup(groupId);

    final entries = <_LedgerEntry>[
      ...expenses.map(_LedgerEntry.expense),
      ...settlements.map(_LedgerEntry.settlement),
    ]..sort((a, b) => b.date.compareTo(a.date));

    if (entries.isEmpty) {
      return const EmptyState(
        icon: Icons.receipt_long,
        title: 'No expenses yet',
        message: 'Tap "Add expense" to record your first shared cost.',
      );
    }

    return ListView.separated(
      padding: const EdgeInsets.only(bottom: 96),
      itemCount: entries.length,
      separatorBuilder: (_, _) => const Divider(height: 1),
      itemBuilder: (context, i) {
        final entry = entries[i];
        if (entry.expense != null) {
          return _ExpenseTile(
            expense: entry.expense!,
            currencyCode: group.currencyCode,
          );
        }
        return _SettlementTile(
          settlement: entry.settlement!,
          currencyCode: group.currencyCode,
        );
      },
    );
  }
}

class _LedgerEntry {
  _LedgerEntry.expense(Expense e)
      : expense = e,
        settlement = null,
        date = e.date;
  _LedgerEntry.settlement(Settlement s)
      : expense = null,
        settlement = s,
        date = s.date;

  final Expense? expense;
  final Settlement? settlement;
  final DateTime date;
}

class _ExpenseTile extends ConsumerWidget {
  const _ExpenseTile({required this.expense, required this.currencyCode});
  final Expense expense;
  final String currencyCode;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(appControllerProvider);
    final theme = Theme.of(context);
    final me = state.currentUserId;
    final myShare = expense.paidByUser(me) - expense.owedByUser(me);
    final category = Categories.byId(expense.categoryId);

    final payerNames = expense.paidBy.keys
        .map((id) => state.displayName(id))
        .join(', ');

    return ListTile(
      leading: CircleAvatar(
        backgroundColor: theme.colorScheme.secondaryContainer,
        child: Icon(category.icon,
            color: theme.colorScheme.onSecondaryContainer, size: 20),
      ),
      title: Text(expense.description,
          style: const TextStyle(fontWeight: FontWeight.w600)),
      subtitle: Text(
        '$payerNames paid ${Money.format(expense.totalCents, code: currencyCode)}',
      ),
      trailing: BalanceLabel(
        netCents: myShare,
        currencyCode: currencyCode,
        youArePositive: 'you lent',
        youAreNegative: 'you borrowed',
        settledText: 'not involved',
      ),
      onTap: () => Navigator.of(context).push(
        MaterialPageRoute<void>(
          builder: (_) => AddExpenseScreen(
            groupId: expense.groupId,
            existing: expense,
          ),
          fullscreenDialog: true,
        ),
      ),
    );
  }
}

class _SettlementTile extends ConsumerWidget {
  const _SettlementTile({required this.settlement, required this.currencyCode});
  final Settlement settlement;
  final String currencyCode;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(appControllerProvider);
    final theme = Theme.of(context);
    return ListTile(
      leading: CircleAvatar(
        backgroundColor: theme.colorScheme.tertiaryContainer,
        child: Icon(Icons.swap_horiz,
            color: theme.colorScheme.onTertiaryContainer, size: 20),
      ),
      title: Text(
        '${state.displayName(settlement.fromUserId)} paid '
        '${state.displayName(settlement.toUserId)}',
      ),
      subtitle: const Text('Payment'),
      trailing: Text(
        Money.format(settlement.amountCents, code: currencyCode),
        style: const TextStyle(
            color: AppTheme.positive, fontWeight: FontWeight.bold),
      ),
      onLongPress: () => ref
          .read(appControllerProvider.notifier)
          .deleteSettlement(settlement.id),
    );
  }
}

class _BalancesTab extends ConsumerWidget {
  const _BalancesTab({required this.groupId});
  final String groupId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(appControllerProvider);
    final group = state.groupById(groupId)!;
    final net = ref.watch(groupNetBalancesProvider(groupId));
    final suggestions = ref.watch(groupSettleUpProvider(groupId));

    return ListView(
      padding: const EdgeInsets.only(bottom: 96),
      children: [
        Padding(
          padding: const EdgeInsets.all(16),
          child: FilledButton.icon(
            onPressed: suggestions.isEmpty
                ? null
                : () => Navigator.of(context).push(
                      MaterialPageRoute<void>(
                        builder: (_) => SettleUpScreen(groupId: groupId),
                      ),
                    ),
            icon: const Icon(Icons.handshake),
            label: Text(suggestions.isEmpty
                ? 'Everyone is settled up'
                : 'Settle up'),
          ),
        ),
        if (suggestions.isNotEmpty) ...[
          const _SectionHeader('Suggested payments'),
          for (final edge in suggestions)
            ListTile(
              leading: const Icon(Icons.arrow_forward),
              title: Text(
                '${state.displayName(edge.fromUserId)} → '
                '${state.displayName(edge.toUserId)}',
              ),
              trailing: Text(
                Money.format(edge.amountCents, code: group.currencyCode),
                style: const TextStyle(fontWeight: FontWeight.bold),
              ),
            ),
        ],
        const _SectionHeader('Member balances'),
        for (final memberId in group.memberIds)
          ListTile(
            leading: state.userById(memberId) == null
                ? const CircleAvatar(child: Icon(Icons.person))
                : UserAvatar(user: state.userById(memberId)!),
            title: Text(state.displayName(memberId)),
            trailing: BalanceLabel(
              netCents: net[memberId] ?? 0,
              currencyCode: group.currencyCode,
              youArePositive: 'gets back',
              youAreNegative: 'owes',
            ),
          ),
      ],
    );
  }
}

class _SectionHeader extends StatelessWidget {
  const _SectionHeader(this.title);
  final String title;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
      child: Text(
        title.toUpperCase(),
        style: Theme.of(context).textTheme.labelMedium?.copyWith(
              color: Theme.of(context).colorScheme.primary,
              fontWeight: FontWeight.bold,
              letterSpacing: 0.5,
            ),
      ),
    );
  }
}
