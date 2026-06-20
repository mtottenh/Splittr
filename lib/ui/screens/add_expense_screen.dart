import 'package:flutter/material.dart' hide Split;
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/categories.dart';
import '../../core/money.dart';
import '../../domain/models/enums.dart';
import '../../domain/models/expense.dart';
import '../../domain/models/split.dart';
import '../../domain/models/user.dart';
import '../../domain/services/split_calculator.dart';
import '../../state/app_state.dart';
import '../../state/providers.dart';

/// Create or edit an expense. Supports four split strategies (equally, exact
/// amounts, percentages, shares), an optional group, a payer and a category.
class AddExpenseScreen extends ConsumerStatefulWidget {
  const AddExpenseScreen({
    super.key,
    this.groupId,
    this.friendId,
    this.existing,
  });

  /// Pre-selected group, if launched from a group.
  final String? groupId;

  /// Pre-selected friend (non-group expense), if launched from a friend.
  final String? friendId;

  /// When set, the screen edits this expense instead of creating one.
  final Expense? existing;

  @override
  ConsumerState<AddExpenseScreen> createState() => _AddExpenseScreenState();
}

class _AddExpenseScreenState extends ConsumerState<AddExpenseScreen> {
  final _formKey = GlobalKey<FormState>();
  final _description = TextEditingController();
  final _amount = TextEditingController();

  /// Per-participant text inputs for exact / percentage / shares modes.
  final Map<String, TextEditingController> _splitInputs = {};

  String? _selectedGroupId;
  String _categoryId = 'general';
  String _payerId = '';
  SplitType _splitType = SplitType.equal;
  DateTime _date = DateTime.now();
  final Set<String> _participants = {};

  bool get _isEditing => widget.existing != null;

  @override
  void initState() {
    super.initState();
    final state = ref.read(appControllerProvider);
    final existing = widget.existing;

    if (existing != null) {
      _description.text = existing.description;
      _amount.text = Money.toMajor(existing.totalCents).toStringAsFixed(2);
      _selectedGroupId = existing.groupId;
      _categoryId = existing.categoryId;
      _payerId = existing.paidBy.keys.first;
      _splitType = existing.splitType;
      _date = existing.date;
      _participants.addAll(existing.participantIds);
    } else {
      _selectedGroupId = widget.groupId;
      _payerId = state.currentUserId;
      _participants.addAll(_availablePeople(state).map((u) => u.id));
      if (widget.friendId != null) {
        _participants
          ..clear()
          ..addAll([state.currentUserId, widget.friendId!]);
      }
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

  List<AppUser> _availablePeople(AppState state) {
    if (_selectedGroupId != null) {
      final group = state.groupById(_selectedGroupId!);
      if (group != null) {
        return [
          for (final id in group.memberIds)
            state.userById(id) ?? AppUser(id: id, name: 'Unknown'),
        ];
      }
    }
    return List<AppUser>.from(state.users);
  }

  String get _currencyCode {
    final state = ref.read(appControllerProvider);
    final group =
        _selectedGroupId == null ? null : state.groupById(_selectedGroupId!);
    return group?.currencyCode ?? 'USD';
  }

  TextEditingController _inputFor(String userId) =>
      _splitInputs.putIfAbsent(userId, () => TextEditingController());

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

    final List<Split> splits;
    try {
      splits = _buildSplits(totalCents);
    } on SplitValidationException catch (e) {
      _error(e.message);
      return;
    }

    final controller = ref.read(appControllerProvider.notifier);
    final paidBy = {_payerId: totalCents};

    try {
      if (_isEditing) {
        await controller.updateExpense(
          widget.existing!.copyWith(
            groupId: _selectedGroupId,
            description: _description.text,
            totalCents: totalCents,
            currencyCode: _currencyCode,
            paidBy: paidBy,
            splits: splits,
            splitType: _splitType,
            categoryId: _categoryId,
            date: _date,
          ),
        );
      } else {
        await controller.addExpense(
          groupId: _selectedGroupId,
          description: _description.text,
          totalCents: totalCents,
          currencyCode: _currencyCode,
          paidBy: paidBy,
          splits: splits,
          splitType: _splitType,
          categoryId: _categoryId,
          date: _date,
        );
      }
    } on StateError catch (_) {
      _error('The amounts entered do not add up to the total.');
      return;
    }

    if (mounted) Navigator.of(context).pop();
  }

  List<Split> _buildSplits(int totalCents) {
    final ids = _participants.toList();
    switch (_splitType) {
      case SplitType.equal:
        return SplitCalculator.equal(totalCents: totalCents, userIds: ids);
      case SplitType.exact:
        final map = <String, int>{
          for (final id in ids)
            id: Money.tryParseToCents(_inputFor(id).text) ?? 0,
        };
        return SplitCalculator.exact(totalCents: totalCents, exactCents: map);
      case SplitType.percentage:
        final map = <String, double>{
          for (final id in ids)
            id: double.tryParse(_inputFor(id).text.trim()) ?? 0,
        };
        return SplitCalculator.percentage(
            totalCents: totalCents, percentages: map);
      case SplitType.shares:
        final map = <String, int>{
          for (final id in ids)
            id: int.tryParse(_inputFor(id).text.trim()) ?? 0,
        };
        return SplitCalculator.shares(totalCents: totalCents, shares: map);
    }
  }

  void _error(String message) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(message)),
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(appControllerProvider);
    final people = _availablePeople(state);
    // Keep the payer valid if the participant set changed.
    if (!people.any((u) => u.id == _payerId) && people.isNotEmpty) {
      _payerId = people.first.id;
    }

    return Scaffold(
      appBar: AppBar(
        title: Text(_isEditing ? 'Edit expense' : 'Add expense'),
        actions: [
          if (_isEditing)
            IconButton(
              icon: const Icon(Icons.delete_outline),
              onPressed: () async {
                await ref
                    .read(appControllerProvider.notifier)
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
            if (widget.groupId == null && widget.friendId == null)
              _groupSelector(state),
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
              decoration: InputDecoration(
                labelText: 'Amount',
                prefixText: '$_currencyCode ',
              ),
              validator: (v) {
                final cents = Money.tryParseToCents(v ?? '');
                if (cents == null || cents <= 0) return 'Enter an amount';
                return null;
              },
            ),
            const SizedBox(height: 16),
            _paidByRow(people, state),
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
            SegmentedButton<SplitType>(
              segments: const [
                ButtonSegment(value: SplitType.equal, label: Text('=')),
                ButtonSegment(value: SplitType.exact, label: Text('1.23')),
                ButtonSegment(value: SplitType.percentage, label: Text('%')),
                ButtonSegment(value: SplitType.shares, label: Text('Shares')),
              ],
              selected: {_splitType},
              onSelectionChanged: (s) => setState(() => _splitType = s.first),
            ),
            const SizedBox(height: 8),
            Text(
              _splitType.label,
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 8),
            ..._buildParticipantRows(people),
            const SizedBox(height: 80),
          ],
        ),
      ),
    );
  }

  Widget _groupSelector(AppState state) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 16),
      child: DropdownButtonFormField<String?>(
        initialValue: _selectedGroupId,
        decoration: const InputDecoration(labelText: 'Group'),
        items: [
          const DropdownMenuItem(value: null, child: Text('Non-group expense')),
          for (final g in state.groups)
            DropdownMenuItem(value: g.id, child: Text('${g.emoji} ${g.name}')),
        ],
        onChanged: (value) => setState(() {
          _selectedGroupId = value;
          final people = _availablePeople(ref.read(appControllerProvider));
          _participants
            ..clear()
            ..addAll(people.map((u) => u.id));
        }),
      ),
    );
  }

  Widget _paidByRow(List<AppUser> people, AppState state) {
    return Row(
      children: [
        const Text('Paid by'),
        const SizedBox(width: 12),
        Expanded(
          child: DropdownButtonFormField<String>(
            initialValue: people.any((u) => u.id == _payerId)
                ? _payerId
                : (people.isEmpty ? null : people.first.id),
            isExpanded: true,
            items: [
              for (final u in people)
                DropdownMenuItem(
                  value: u.id,
                  child: Text(u.id == state.currentUserId ? 'You' : u.name),
                ),
            ],
            onChanged: (value) =>
                setState(() => _payerId = value ?? _payerId),
          ),
        ),
      ],
    );
  }

  List<Widget> _buildParticipantRows(List<AppUser> people) {
    final totalCents = Money.tryParseToCents(_amount.text) ?? 0;
    return [
      for (final user in people)
        _ParticipantRow(
          user: user,
          isCurrentUser:
              user.id == ref.read(appControllerProvider).currentUserId,
          selected: _participants.contains(user.id),
          splitType: _splitType,
          controller: _inputFor(user.id),
          currencyCode: _currencyCode,
          equalShareCents: _participants.isEmpty
              ? 0
              : totalCents ~/ _participants.length,
          onToggle: (checked) => setState(() {
            if (checked) {
              _participants.add(user.id);
            } else {
              _participants.remove(user.id);
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
    required this.user,
    required this.isCurrentUser,
    required this.selected,
    required this.splitType,
    required this.controller,
    required this.currencyCode,
    required this.equalShareCents,
    required this.onToggle,
    required this.onInputChanged,
  });

  final AppUser user;
  final bool isCurrentUser;
  final bool selected;
  final SplitType splitType;
  final TextEditingController controller;
  final String currencyCode;
  final int equalShareCents;
  final ValueChanged<bool> onToggle;
  final VoidCallback onInputChanged;

  @override
  Widget build(BuildContext context) {
    Widget? trailing;
    if (selected) {
      switch (splitType) {
        case SplitType.equal:
          trailing = Text(Money.format(equalShareCents, code: currencyCode));
        case SplitType.exact:
          trailing = _input(suffix: currencyCode);
        case SplitType.percentage:
          trailing = _input(suffix: '%');
        case SplitType.shares:
          trailing = _input(suffix: 'sh');
      }
    }

    return CheckboxListTile(
      contentPadding: EdgeInsets.zero,
      controlAffinity: ListTileControlAffinity.leading,
      value: selected,
      onChanged: (v) => onToggle(v ?? false),
      secondary: trailing,
      title: Text(isCurrentUser ? 'You' : user.name),
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
        decoration: InputDecoration(
          isDense: true,
          suffixText: suffix,
        ),
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
