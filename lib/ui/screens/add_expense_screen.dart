import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/categories.dart';
import '../../core/money.dart';
import '../../src/rust/dto.dart';
import '../../state/providers.dart';

/// How an expense is divided. Maps onto the engine's split plans: [equal] and
/// [exact] directly, [shares] onto a weighted plan.
enum SplitMode { equal, exact, shares }

extension on SplitMode {
  String get hint => switch (this) {
        SplitMode.equal => 'Split equally between everyone selected.',
        SplitMode.exact => 'Enter the exact amount each person owes.',
        SplitMode.shares => 'Enter shares; the cost is divided in proportion.',
      };
}

/// Create or edit an expense. Works in three modes: within a [group], as a
/// non-group expense with a friend ([friendId]/[friendName]), or editing an
/// [existing] one. Business rules (balancing, split maths) are enforced by the
/// Rust engine — this screen just gathers input.
class AddExpenseScreen extends ConsumerStatefulWidget {
  const AddExpenseScreen({
    super.key,
    this.group,
    this.friendId,
    this.friendName,
    this.existing,
  });

  /// Set for a group expense.
  final GroupDetailDto? group;

  /// Set (with [friendName]) for a new non-group expense with a friend (#31).
  final String? friendId;
  final String? friendName;

  /// When set, the screen edits this expense instead of creating one.
  final ExpenseViewDto? existing;

  @override
  ConsumerState<AddExpenseScreen> createState() => _AddExpenseScreenState();
}

class _AddExpenseScreenState extends ConsumerState<AddExpenseScreen> {
  final _formKey = GlobalKey<FormState>();
  final _description = TextEditingController();
  final _amount = TextEditingController();

  /// Per-participant inputs for exact / shares modes.
  final Map<String, TextEditingController> _splitInputs = {};

  String _categoryId = 'general';
  String _payerId = '';
  SplitMode _splitMode = SplitMode.equal;
  DateTime _date = DateTime.now();
  bool _draft = false;
  final Set<String> _participants = {};

  bool get _isEditing => widget.existing != null;

  /// The participant universe for this expense, resolved from whichever mode the
  /// screen was opened in.
  late final List<MemberBalanceDto> _members = _resolveMembers();

  /// The group this expense belongs to, or null for a non-group expense.
  String? get _groupId => widget.group?.id;

  List<MemberBalanceDto> _resolveMembers() {
    final data = ref.read(appProvider).requireValue;
    if (widget.group != null) return widget.group!.members;
    if (widget.existing != null) {
      // Editing: reconstruct the participants from the stored expense.
      final byId = <String, String>{};
      for (final s in widget.existing!.splits) {
        byId[s.userId] = s.name;
      }
      for (final p in widget.existing!.paidBy) {
        byId[p.userId] = p.name;
      }
      return [
        for (final e in byId.entries)
          MemberBalanceDto(userId: e.key, name: e.value, netCents: 0),
      ];
    }
    // New non-group expense: just me and the friend.
    return [
      MemberBalanceDto(userId: data.myUserId, name: data.myName, netCents: 0),
      MemberBalanceDto(
          userId: widget.friendId!, name: widget.friendName!, netCents: 0),
    ];
  }

  @override
  void initState() {
    super.initState();
    final existing = widget.existing;
    if (existing != null) {
      _description.text = existing.description;
      _amount.text = Money.toMajor(existing.totalCents).toStringAsFixed(2);
      _categoryId = existing.category;
      _payerId = existing.paidBy.isEmpty ? '' : existing.paidBy.first.userId;
      _date = DateTime.fromMillisecondsSinceEpoch(existing.dateMs);
      _participants.addAll(existing.splits.map((s) => s.userId));
    } else {
      _payerId = ref.read(appProvider).requireValue.myUserId;
      _participants.addAll(_members.map((m) => m.userId));
    }
    if (!_members.any((m) => m.userId == _payerId) && _members.isNotEmpty) {
      _payerId = _members.first.userId;
    }
  }

  @override
  void dispose() {
    _description.dispose();
    _amount.dispose();
    for (final c in _splitInputs.values) {
      c.dispose();
    }
    super.dispose();
  }

  TextEditingController _inputFor(String userId) =>
      _splitInputs.putIfAbsent(userId, () => TextEditingController());

  SplitPlanDto _buildPlan(int totalCents) {
    final ids = _participants.toList();
    switch (_splitMode) {
      case SplitMode.equal:
        return SplitPlanDto.equal(participants: ids);
      case SplitMode.exact:
        return SplitPlanDto.exact(
          amounts: [
            for (final id in ids)
              Payer(
                userId: id,
                cents: Money.tryParseToCents(_inputFor(id).text) ?? 0,
              ),
          ],
        );
      case SplitMode.shares:
        return SplitPlanDto.weighted(
          weights: [
            for (final id in ids)
              Weight(
                userId: id,
                weight: BigInt.from(int.tryParse(_inputFor(id).text.trim()) ?? 0),
              ),
          ],
        );
    }
  }

  Future<void> _save() async {
    if (!_formKey.currentState!.validate()) return;
    final totalCents = Money.tryParseToCents(_amount.text);
    if (totalCents == null || totalCents <= 0) {
      _error('Enter a valid amount.');
      return;
    }
    if (_participants.isEmpty) {
      _error('Select at least one participant.');
      return;
    }

    final input = ExpenseInput(
      groupId: _groupId,
      description: _description.text.trim(),
      paidBy: [Payer(userId: _payerId, cents: totalCents)],
      totalCents: totalCents,
      split: _buildPlan(totalCents),
      category: _categoryId,
      dateMs: _date.millisecondsSinceEpoch,
      draft: _isEditing ? false : _draft,
    );

    final notifier = ref.read(appProvider.notifier);
    try {
      if (_isEditing) {
        await notifier.editExpense(widget.existing!.id, input);
      } else {
        await notifier.addExpense(input);
      }
    } catch (e) {
      _error('Could not save: the amounts must add up to the total.');
      return;
    }
    if (mounted) Navigator.of(context).pop();
  }

  void _error(String message) {
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(message)));
  }

  String _label(MemberBalanceDto m) =>
      m.userId == ref.read(appProvider).requireValue.myUserId ? 'You' : m.name;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(_isEditing ? 'Edit expense' : 'Add expense'),
        actions: [
          if (_isEditing && !widget.existing!.published)
            IconButton(
              tooltip: 'Publish',
              icon: const Icon(Icons.publish_outlined),
              onPressed: () async {
                await ref
                    .read(appProvider.notifier)
                    .publishExpense(widget.existing!.id);
                if (context.mounted) Navigator.of(context).pop();
              },
            ),
          if (_isEditing)
            IconButton(
              tooltip: 'Delete',
              icon: const Icon(Icons.delete_outline),
              onPressed: () async {
                await ref
                    .read(appProvider.notifier)
                    .deleteExpense(widget.existing!.id);
                if (context.mounted) Navigator.of(context).pop();
              },
            ),
          TextButton(onPressed: _save, child: const Text('Save')),
        ],
      ),
      body: Form(
        key: _formKey,
        child: ListView(
          padding: const EdgeInsets.all(16),
          children: [
            Row(
              children: [
                _CategoryButton(
                  categoryId: _categoryId,
                  onChanged: (id) => setState(() => _categoryId = id),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: TextFormField(
                    controller: _description,
                    textCapitalization: TextCapitalization.sentences,
                    decoration: const InputDecoration(
                      labelText: 'Description',
                      hintText: 'e.g. Dinner',
                    ),
                    validator: (v) => (v == null || v.trim().isEmpty)
                        ? 'Enter a description'
                        : null,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),
            TextFormField(
              controller: _amount,
              keyboardType:
                  const TextInputType.numberWithOptions(decimal: true),
              inputFormatters: [
                FilteringTextInputFormatter.allow(RegExp(r'[0-9.,]')),
              ],
              decoration: const InputDecoration(
                labelText: 'Amount',
                prefixText: 'USD ',
              ),
              onChanged: (_) => setState(() {}),
              validator: (v) {
                final cents = Money.tryParseToCents(v ?? '');
                if (cents == null || cents <= 0) return 'Enter an amount';
                return null;
              },
            ),
            const SizedBox(height: 16),
            _paidByRow(),
            const SizedBox(height: 8),
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.calendar_today),
              title: const Text('Date'),
              trailing: Text(
                '${_date.year}-${_date.month.toString().padLeft(2, '0')}-'
                '${_date.day.toString().padLeft(2, '0')}',
              ),
              onTap: _pickDate,
            ),
            const Divider(height: 24),
            Text('Split', style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 8),
            SegmentedButton<SplitMode>(
              segments: const [
                ButtonSegment(value: SplitMode.equal, label: Text('=')),
                ButtonSegment(value: SplitMode.exact, label: Text('1.23')),
                ButtonSegment(value: SplitMode.shares, label: Text('Shares')),
              ],
              selected: {_splitMode},
              onSelectionChanged: (s) => setState(() => _splitMode = s.first),
            ),
            const SizedBox(height: 8),
            Text(_splitMode.hint,
                style: Theme.of(context).textTheme.bodySmall),
            const SizedBox(height: 8),
            ..._participantRows(),
            if (!_isEditing) ...[
              const Divider(height: 24),
              CheckboxListTile(
                contentPadding: EdgeInsets.zero,
                value: _draft,
                onChanged: (v) => setState(() => _draft = v ?? false),
                title: const Text('Save as draft'),
                subtitle:
                    const Text("Keep private; doesn't count until published"),
              ),
            ],
            const SizedBox(height: 80),
          ],
        ),
      ),
    );
  }

  Widget _paidByRow() {
    return Row(
      children: [
        const Text('Paid by'),
        const SizedBox(width: 12),
        Expanded(
          child: DropdownButtonFormField<String>(
            initialValue: _members.any((m) => m.userId == _payerId)
                ? _payerId
                : (_members.isEmpty ? null : _members.first.userId),
            isExpanded: true,
            items: [
              for (final m in _members)
                DropdownMenuItem(value: m.userId, child: Text(_label(m))),
            ],
            onChanged: (value) =>
                setState(() => _payerId = value ?? _payerId),
          ),
        ),
      ],
    );
  }

  List<Widget> _participantRows() {
    final totalCents = Money.tryParseToCents(_amount.text) ?? 0;
    final equalShare =
        _participants.isEmpty ? 0 : totalCents ~/ _participants.length;
    return [
      for (final m in _members)
        _ParticipantRow(
          label: _label(m),
          selected: _participants.contains(m.userId),
          mode: _splitMode,
          controller: _inputFor(m.userId),
          equalShareCents: equalShare,
          onToggle: (checked) => setState(() {
            if (checked) {
              _participants.add(m.userId);
            } else {
              _participants.remove(m.userId);
            }
          }),
          onInputChanged: () => setState(() {}),
        ),
    ];
  }

  Future<void> _pickDate() async {
    final picked = await showDatePicker(
      context: context,
      initialDate: _date,
      firstDate: DateTime(2015),
      lastDate: DateTime.now().add(const Duration(days: 1)),
    );
    if (picked != null) setState(() => _date = picked);
  }
}

class _ParticipantRow extends StatelessWidget {
  const _ParticipantRow({
    required this.label,
    required this.selected,
    required this.mode,
    required this.controller,
    required this.equalShareCents,
    required this.onToggle,
    required this.onInputChanged,
  });

  final String label;
  final bool selected;
  final SplitMode mode;
  final TextEditingController controller;
  final int equalShareCents;
  final ValueChanged<bool> onToggle;
  final VoidCallback onInputChanged;

  @override
  Widget build(BuildContext context) {
    Widget? trailing;
    if (selected) {
      switch (mode) {
        case SplitMode.equal:
          trailing = Text(Money.format(equalShareCents));
        case SplitMode.exact:
          trailing = _input(suffix: 'USD');
        case SplitMode.shares:
          trailing = _input(suffix: 'sh');
      }
    }

    return CheckboxListTile(
      contentPadding: EdgeInsets.zero,
      controlAffinity: ListTileControlAffinity.leading,
      value: selected,
      onChanged: (v) => onToggle(v ?? false),
      secondary: trailing,
      title: Text(label),
    );
  }

  Widget _input({required String suffix}) {
    return SizedBox(
      width: 96,
      child: TextField(
        controller: controller,
        textAlign: TextAlign.end,
        keyboardType: const TextInputType.numberWithOptions(decimal: true),
        inputFormatters: [
          FilteringTextInputFormatter.allow(RegExp(r'[0-9.,]')),
        ],
        decoration: InputDecoration(isDense: true, suffixText: suffix),
        onChanged: (_) => onInputChanged(),
      ),
    );
  }
}

class _CategoryButton extends StatelessWidget {
  const _CategoryButton({required this.categoryId, required this.onChanged});
  final String categoryId;
  final ValueChanged<String> onChanged;

  @override
  Widget build(BuildContext context) {
    final category = Categories.byId(categoryId);
    return InkWell(
      borderRadius: BorderRadius.circular(12),
      onTap: () async {
        final picked = await showModalBottomSheet<String>(
          context: context,
          builder: (context) => SafeArea(
            child: GridView.count(
              crossAxisCount: 4,
              shrinkWrap: true,
              padding: const EdgeInsets.all(16),
              children: [
                for (final c in Categories.all)
                  InkWell(
                    onTap: () => Navigator.of(context).pop(c.id),
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(c.icon),
                        const SizedBox(height: 4),
                        Text(c.label,
                            style: Theme.of(context).textTheme.labelSmall,
                            textAlign: TextAlign.center),
                      ],
                    ),
                  ),
              ],
            ),
          ),
        );
        if (picked != null) onChanged(picked);
      },
      child: Container(
        width: 56,
        height: 56,
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.secondaryContainer,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Icon(category.icon,
            color: Theme.of(context).colorScheme.onSecondaryContainer),
      ),
    );
  }
}
