import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/categories.dart';
import '../../core/money.dart';
import '../../core/theme.dart';
import '../../src/rust/dto.dart';
import '../../state/providers.dart';
import '../widgets/add_person_dialog.dart';
import '../widgets/balance_label.dart';
import '../widgets/empty_state.dart';
import '../widgets/invite_sheet.dart';
import '../widgets/user_avatar.dart';
import 'add_expense_screen.dart';
import 'settle_up_screen.dart';

/// Detailed view of a single group: its ledger and its balances.
class GroupDetailScreen extends ConsumerWidget {
  const GroupDetailScreen({super.key, required this.groupId});

  final String groupId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final detail = ref.watch(groupDetailProvider(groupId));

    return detail.when(
      loading: () =>
          const Scaffold(body: Center(child: CircularProgressIndicator())),
      error: (e, _) => Scaffold(body: Center(child: Text('Error: $e'))),
      data: (group) {
        if (group == null) {
          return const Scaffold(body: Center(child: Text('Group not found')));
        }
        return _GroupDetail(group: group);
      },
    );
  }
}

class _GroupDetail extends ConsumerWidget {
  const _GroupDetail({required this.group});
  final GroupDetailDto group;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return DefaultTabController(
      length: 2,
      child: Scaffold(
        appBar: AppBar(
          title: Text(group.name, overflow: TextOverflow.ellipsis),
          actions: [
            IconButton(
              tooltip: 'Add member',
              icon: const Icon(Icons.person_add_alt),
              onPressed: () => _addMember(context, ref),
            ),
            IconButton(
              tooltip: 'Invite to group',
              icon: const Icon(Icons.qr_code),
              onPressed: () => showGroupInvite(context, ref,
                  groupId: group.id, groupName: group.name),
            ),
            PopupMenuButton<String>(
              onSelected: (v) {
                if (v == 'rename') _rename(context, ref);
                if (v == 'close') _closePeriod(context, ref);
                if (v == 'reopen') _reopenPeriod(ref);
              },
              itemBuilder: (context) => const [
                PopupMenuItem(value: 'rename', child: Text('Rename group…')),
                PopupMenuItem(
                  value: 'close',
                  child: Text('Close period up to a date…'),
                ),
                PopupMenuItem(value: 'reopen', child: Text('Reopen all periods')),
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
              builder: (_) => AddExpenseScreen(group: group),
              fullscreenDialog: true,
            ),
          ),
          icon: const Icon(Icons.add),
          label: const Text('Add expense'),
        ),
        body: TabBarView(
          children: [
            _ExpensesTab(group: group),
            _BalancesTab(group: group),
          ],
        ),
      ),
    );
  }

  Future<void> _addMember(BuildContext context, WidgetRef ref) async {
    final memberIds = group.members.map((m) => m.userId).toSet();
    final candidates = ref
        .read(appProvider)
        .requireValue
        .friends
        .where((f) => !memberIds.contains(f.userId))
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
                final id = await showAddPersonDialog(context, ref);
                if (context.mounted) Navigator.of(context).pop(id);
              },
            ),
            const Divider(),
            for (final f in candidates)
              ListTile(
                leading: UserAvatar(name: f.name, id: f.userId),
                title: Text(f.name),
                onTap: () => Navigator.of(context).pop(f.userId),
              ),
          ],
        ),
      ),
    );
    if (selected != null) {
      await ref.read(appProvider.notifier).addMember(group.id, selected);
    }
  }

  Future<void> _rename(BuildContext context, WidgetRef ref) async {
    final controller = TextEditingController(text: group.name);
    final name = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Rename group'),
        content: TextField(
          controller: controller,
          autofocus: true,
          textCapitalization: TextCapitalization.words,
          decoration: const InputDecoration(labelText: 'Group name'),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(controller.text.trim()),
            child: const Text('Save'),
          ),
        ],
      ),
    );
    controller.dispose();
    if (name != null && name.isNotEmpty) {
      await ref.read(appProvider.notifier).renameGroup(group.id, name);
    }
  }

  Future<void> _closePeriod(BuildContext context, WidgetRef ref) async {
    final picked = await showDatePicker(
      context: context,
      initialDate: DateTime.now(),
      firstDate: DateTime(2015),
      lastDate: DateTime.now().add(const Duration(days: 1)),
      helpText: 'Freeze expenses up to and including',
    );
    if (picked == null) return;
    // Include the whole chosen day.
    final until = DateTime(picked.year, picked.month, picked.day, 23, 59, 59)
        .millisecondsSinceEpoch;
    await ref.read(appProvider.notifier).setClosedPeriod(group.id, until);
  }

  Future<void> _reopenPeriod(WidgetRef ref) =>
      ref.read(appProvider.notifier).setClosedPeriod(group.id, 0);
}

/// A ledger entry is an expense or a settlement; this unifies them for the list.
class _LedgerEntry {
  _LedgerEntry.expense(ExpenseViewDto e)
      : expense = e,
        settlement = null,
        sortKey = e.dateMs;
  // Settlements have no stored timestamp; they sort after dated expenses.
  _LedgerEntry.settlement(SettlementViewDto s)
      : expense = null,
        settlement = s,
        sortKey = -1;

  final ExpenseViewDto? expense;
  final SettlementViewDto? settlement;
  final int sortKey;
}

class _ExpensesTab extends ConsumerWidget {
  const _ExpensesTab({required this.group});
  final GroupDetailDto group;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final entries = <_LedgerEntry>[
      ...group.expenses.map(_LedgerEntry.expense),
      ...group.settlements.map(_LedgerEntry.settlement),
    ]..sort((a, b) => b.sortKey.compareTo(a.sortKey));

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
          return _ExpenseTile(group: group, expense: entry.expense!);
        }
        return _SettlementTile(
            settlement: entry.settlement!, currency: group.currency);
      },
    );
  }
}

class _ExpenseTile extends StatelessWidget {
  const _ExpenseTile({required this.group, required this.expense});
  final GroupDetailDto group;
  final ExpenseViewDto expense;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final category = Categories.byId(expense.category);
    final payerNames = expense.paidBy.map((p) => p.name).join(', ');

    return ListTile(
      leading: CircleAvatar(
        backgroundColor: theme.colorScheme.secondaryContainer,
        child: Icon(category.icon,
            color: theme.colorScheme.onSecondaryContainer, size: 20),
      ),
      title: Row(
        children: [
          Flexible(
            child: Text(expense.description,
                style: const TextStyle(fontWeight: FontWeight.w600),
                overflow: TextOverflow.ellipsis),
          ),
          if (!expense.published) ...[
            const SizedBox(width: 8),
            const _DraftChip(),
          ],
        ],
      ),
      subtitle: Text(
        '$payerNames paid '
        '${Money.format(expense.totalCents, code: expense.currency)}'
        '${_originalSuffix(expense)}',
      ),
      trailing: BalanceLabel(
        netCents: expense.myNetCents,
        currencyCode: expense.currency,
        youArePositive: 'you lent',
        youAreNegative: 'you borrowed',
        settledText: 'not involved',
      ),
      onTap: () => Navigator.of(context).push(
        MaterialPageRoute<void>(
          builder: (_) => AddExpenseScreen(group: group, existing: expense),
          fullscreenDialog: true,
        ),
      ),
    );
  }

  /// "(€30.00)" suffix when the expense was entered in another currency (#3).
  String _originalSuffix(ExpenseViewDto e) {
    final o = e.original;
    if (o == null) return '';
    return ' (${Money.format(o.amountCents, code: o.currency)})';
  }
}

class _DraftChip extends StatelessWidget {
  const _DraftChip();

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: scheme.tertiaryContainer,
        borderRadius: BorderRadius.circular(6),
      ),
      child: Text('DRAFT',
          style: TextStyle(
            fontSize: 10,
            fontWeight: FontWeight.bold,
            color: scheme.onTertiaryContainer,
          )),
    );
  }
}

class _SettlementTile extends ConsumerWidget {
  const _SettlementTile({required this.settlement, required this.currency});
  final SettlementViewDto settlement;
  final String currency;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    return ListTile(
      leading: CircleAvatar(
        backgroundColor: theme.colorScheme.tertiaryContainer,
        child: Icon(Icons.swap_horiz,
            color: theme.colorScheme.onTertiaryContainer, size: 20),
      ),
      title: Text('${settlement.fromName} paid ${settlement.toName}'),
      subtitle: const Text('Payment · long-press to remove'),
      trailing: Text(
        Money.format(settlement.amountCents, code: currency),
        style: const TextStyle(
            color: AppTheme.positive, fontWeight: FontWeight.bold),
      ),
      onLongPress: () =>
          ref.read(appProvider.notifier).deleteSettlement(settlement.id),
    );
  }
}

class _BalancesTab extends StatelessWidget {
  const _BalancesTab({required this.group});
  final GroupDetailDto group;

  @override
  Widget build(BuildContext context) {
    final settled = group.settleUp.isEmpty;
    return ListView(
      padding: const EdgeInsets.only(bottom: 96),
      children: [
        Padding(
          padding: const EdgeInsets.all(16),
          child: FilledButton.icon(
            onPressed: settled
                ? null
                : () => Navigator.of(context).push(
                      MaterialPageRoute<void>(
                        builder: (_) => SettleUpScreen(group: group),
                      ),
                    ),
            icon: const Icon(Icons.handshake),
            label: Text(settled ? 'Everyone is settled up' : 'Settle up'),
          ),
        ),
        if (!settled) ...[
          const _SectionHeader('Suggested payments'),
          for (final t in group.settleUp)
            ListTile(
              leading: const Icon(Icons.arrow_forward),
              title: Text('${_name(group, t.from)} → ${_name(group, t.to)}'),
              trailing: Text(
                Money.format(t.amountCents, code: group.currency),
                style: const TextStyle(fontWeight: FontWeight.bold),
              ),
            ),
        ],
        const _SectionHeader('Member balances'),
        for (final m in group.members)
          ListTile(
            leading: UserAvatar(name: m.name, id: m.userId),
            title: Text(m.name),
            trailing: BalanceLabel(
              netCents: m.netCents,
              currencyCode: group.currency,
              youArePositive: 'gets back',
              youAreNegative: 'owes',
            ),
          ),
      ],
    );
  }

  String _name(GroupDetailDto group, String userId) => group.members
      .firstWhere(
        (m) => m.userId == userId,
        orElse: () => MemberBalanceDto(userId: userId, name: userId, netCents: 0),
      )
      .name;
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
