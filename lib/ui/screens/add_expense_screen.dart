import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/categories.dart';
import '../../core/currencies.dart';
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

  /// Per-payer amount inputs, used when [_multiplePayers] is on.
  final Map<String, TextEditingController> _payerInputs = {};

  String _categoryId = 'general';
  String _payerId = '';
  bool _multiplePayers = false;
  SplitMode _splitMode = SplitMode.equal;
  DateTime _date = DateTime.now();
  bool _draft = false;
  final Set<String> _participants = {};

  // --- multi-currency (#3) ---
  late String _currency;
  final _rate = TextEditingController();
  int _currencyMinor = 2;
  int _baseMinor = 2;
  bool _loadingRate = false;

  bool get _isEditing => widget.existing != null;

  /// The currency balances are kept in: the group's, or USD for a non-group.
  String get _baseCurrency => widget.group?.currency ?? 'USD';

  /// Whether the expense is being entered in a non-base currency.
  bool get _foreign => _currency != _baseCurrency;

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
      // Pre-fill multi-payer mode when the expense had more than one payer.
      if (existing.paidBy.length > 1) {
        _multiplePayers = true;
        for (final p in existing.paidBy) {
          _payerInput(p.userId).text =
              Money.toMajor(p.cents).toStringAsFixed(2);
        }
      }
    } else {
      _payerId = ref.read(appProvider).requireValue.myUserId;
      _participants.addAll(_members.map((m) => m.userId));
    }
    if (!_members.any((m) => m.userId == _payerId) && _members.isNotEmpty) {
      _payerId = _members.first.userId;
    }
    _currency = _baseCurrency;
    // Load minor-unit metadata (and a rate if the currency differs).
    Future.microtask(_refreshCurrencyMeta);
  }

  @override
  void dispose() {
    _description.dispose();
    _amount.dispose();
    _rate.dispose();
    for (final c in _splitInputs.values) {
      c.dispose();
    }
    for (final c in _payerInputs.values) {
      c.dispose();
    }
    super.dispose();
  }

  /// Fetch the selected/base currencies' minor units and, when they differ, a
  /// live exchange rate to pre-fill the (editable) rate field.
  Future<void> _refreshCurrencyMeta() async {
    final notifier = ref.read(appProvider.notifier);
    final cm = await notifier.currencyMinorUnits(_currency);
    final bm = await notifier.currencyMinorUnits(_baseCurrency);
    double? rate;
    if (_foreign) {
      if (mounted) setState(() => _loadingRate = true);
      rate = await ref
          .read(exchangeRateProvider)
          .rate(base: _currency, quote: _baseCurrency, on: _date);
    }
    if (!mounted) return;
    setState(() {
      _currencyMinor = cm;
      _baseMinor = bm;
      _loadingRate = false;
      if (_foreign && rate != null) _rate.text = rate.toString();
      // Re-format the edit pre-fill now the base minor units are known (initState
      // could only assume 2): a ¥1000 expense must show "1000", not "10.00".
      if (_isEditing && !_didEditPrefill) {
        _didEditPrefill = true;
        final e = widget.existing!;
        _amount.text =
            Money.toMajor(e.totalCents, minorUnits: bm).toStringAsFixed(bm);
        if (e.paidBy.length > 1) {
          for (final p in e.paidBy) {
            _payerInput(p.userId).text =
                Money.toMajor(p.cents, minorUnits: bm).toStringAsFixed(bm);
          }
        }
      }
    });
  }

  /// Guards the one-time edit pre-fill reformat in [_refreshCurrencyMeta].
  bool _didEditPrefill = false;

  TextEditingController _inputFor(String userId) =>
      _splitInputs.putIfAbsent(userId, () => TextEditingController());

  TextEditingController _payerInput(String userId) =>
      _payerInputs.putIfAbsent(userId, () => TextEditingController());

  /// Build the payer list: a single payer for the whole total, or the
  /// per-payer amounts when multi-payer mode is on. The engine validates that
  /// these sum to the total.
  List<Payer> _buildPayers(int totalCents) {
    if (!_multiplePayers) {
      return [Payer(userId: _payerId, cents: totalCents)];
    }
    return [
      for (final m in _members)
        if ((Money.tryParseToCents(_payerInput(m.userId).text, minorUnits: _baseMinor) ?? 0) > 0)
          Payer(
            userId: m.userId,
            cents: Money.tryParseToCents(_payerInput(m.userId).text, minorUnits: _baseMinor)!,
          ),
    ];
  }

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
                cents: Money.tryParseToCents(_inputFor(id).text, minorUnits: _baseMinor) ?? 0,
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
    if (_participants.isEmpty) {
      _error('Select at least one participant.');
      return;
    }

    final notifier = ref.read(appProvider.notifier);

    // Resolve the base-currency total and (for a foreign currency) the original.
    final int totalCents;
    OriginalAmountDto? original;
    if (_foreign) {
      final orig = Money.tryParseToCents(_amount.text, minorUnits: _currencyMinor);
      if (orig == null || orig <= 0) {
        _error('Enter a valid amount.');
        return;
      }
      final rate = double.tryParse(_rate.text.trim());
      if (rate == null || rate <= 0) {
        _error('Enter an exchange rate.');
        return;
      }
      final rateMicro = (rate * 1000000).round();
      totalCents = await notifier.convertCurrency(
        amountCents: orig,
        rateMicro: rateMicro,
        from: _currency,
        to: _baseCurrency,
      );
      original = OriginalAmountDto(
        currency: _currency,
        amountCents: orig,
        rateMicro: rateMicro,
      );
    } else {
      final t = Money.tryParseToCents(_amount.text, minorUnits: _baseMinor);
      if (t == null || t <= 0) {
        _error('Enter a valid amount.');
        return;
      }
      totalCents = t;
    }

    // In foreign mode a single payer covers the converted total; the split is
    // computed by the engine on the base total.
    final payers =
        _foreign ? [Payer(userId: _payerId, cents: totalCents)] : _buildPayers(totalCents);
    if (_multiplePayers &&
        !_foreign &&
        payers.fold(0, (sum, p) => sum + p.cents) != totalCents) {
      _error('Payments must total ${Money.format(totalCents, code: _baseCurrency)}.');
      return;
    }

    final input = ExpenseInput(
      groupId: _groupId,
      description: _description.text.trim(),
      paidBy: payers,
      totalCents: totalCents,
      split: _buildPlan(totalCents),
      category: _categoryId,
      dateMs: _date.millisecondsSinceEpoch,
      draft: _isEditing ? false : _draft,
      original: original,
    );

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
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: TextFormField(
                    controller: _amount,
                    keyboardType:
                        const TextInputType.numberWithOptions(decimal: true),
                    inputFormatters: [
                      FilteringTextInputFormatter.allow(RegExp(r'[0-9.,]')),
                    ],
                    decoration: InputDecoration(
                      labelText: 'Amount',
                      prefixText: '$_currency ',
                    ),
                    onChanged: (_) => setState(() {}),
                    validator: (v) =>
                        (Money.tryParseToCents(v ?? '', minorUnits: _currencyMinor) ?? 0) <= 0
                            ? 'Enter an amount'
                            : null,
                  ),
                ),
                const SizedBox(width: 12),
                DropdownButton<String>(
                  value: kCurrencies.contains(_currency) ? _currency : null,
                  hint: Text(_currency),
                  items: [
                    for (final c in {_baseCurrency, ...kCurrencies})
                      DropdownMenuItem(value: c, child: Text(c)),
                  ],
                  onChanged: (v) {
                    if (v == null) return;
                    setState(() {
                      _currency = v;
                      _rate.clear();
                      // Foreign currencies use a single payer + proportional split.
                      if (_foreign) {
                        _multiplePayers = false;
                        if (_splitMode == SplitMode.exact) {
                          _splitMode = SplitMode.equal;
                        }
                      }
                    });
                    _refreshCurrencyMeta();
                  },
                ),
              ],
            ),
            if (_foreign) _foreignRateRow(),
            const SizedBox(height: 16),
            _payersSection(),
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
              segments: [
                const ButtonSegment(value: SplitMode.equal, label: Text('=')),
                // Exact amounts need a single currency; offered in base only.
                if (!_foreign)
                  const ButtonSegment(
                      value: SplitMode.exact, label: Text('1.23')),
                const ButtonSegment(
                    value: SplitMode.shares, label: Text('Shares')),
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

  /// Editable exchange rate (pre-filled from a live lookup) plus a converted
  /// preview. The conversion itself runs in the engine (no duplicated maths).
  Widget _foreignRateRow() {
    final orig = Money.tryParseToCents(_amount.text, minorUnits: _currencyMinor);
    final rate = double.tryParse(_rate.text.trim());
    final bodySmall = Theme.of(context).textTheme.bodySmall;
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(child: Text('1 $_currency =')),
              SizedBox(
                width: 150,
                child: TextField(
                  controller: _rate,
                  keyboardType:
                      const TextInputType.numberWithOptions(decimal: true),
                  inputFormatters: [
                    FilteringTextInputFormatter.allow(RegExp(r'[0-9.]')),
                  ],
                  decoration: InputDecoration(
                    isDense: true,
                    suffixText: _baseCurrency,
                  ),
                  onChanged: (_) => setState(() {}),
                ),
              ),
            ],
          ),
          const SizedBox(height: 4),
          if (_loadingRate)
            Text('Fetching rate…', style: bodySmall)
          else if (orig != null && orig > 0 && rate != null && rate > 0)
            FutureBuilder<int>(
              future: ref.read(appProvider.notifier).convertCurrency(
                    amountCents: orig,
                    rateMicro: (rate * 1000000).round(),
                    from: _currency,
                    to: _baseCurrency,
                  ),
              builder: (context, snap) => Text(
                snap.hasData
                    ? '≈ ${Money.format(snap.data!, code: _baseCurrency)}'
                    : '',
                style: bodySmall,
              ),
            )
          else
            Text('Enter a rate to convert', style: bodySmall),
        ],
      ),
    );
  }

  /// "Paid by": a single payer (dropdown) or, in multi-payer mode, a per-person
  /// amount list that must sum to the total (the engine enforces it too).
  Widget _payersSection() {
    if (!_multiplePayers) {
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
              onChanged: (value) => setState(() => _payerId = value ?? _payerId),
            ),
          ),
          // Multi-payer needs a single currency; offered in base mode only.
          if (!_foreign)
            TextButton(
              onPressed: () => setState(() => _multiplePayers = true),
              child: const Text('Multiple'),
            ),
        ],
      );
    }

    final total = Money.tryParseToCents(_amount.text, minorUnits: _baseMinor) ?? 0;
    final entered = _members.fold<int>(
      0,
      (sum, m) => sum + (Money.tryParseToCents(_payerInput(m.userId).text, minorUnits: _baseMinor) ?? 0),
    );
    final balanced = entered == total;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            const Text('Paid by'),
            const Spacer(),
            TextButton(
              onPressed: () => setState(() => _multiplePayers = false),
              child: const Text('Single payer'),
            ),
          ],
        ),
        for (final m in _members)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 2),
            child: Row(
              children: [
                Expanded(child: Text(_label(m))),
                SizedBox(
                  width: 110,
                  child: TextField(
                    controller: _payerInput(m.userId),
                    textAlign: TextAlign.end,
                    keyboardType:
                        const TextInputType.numberWithOptions(decimal: true),
                    inputFormatters: [
                      FilteringTextInputFormatter.allow(RegExp(r'[0-9.,]')),
                    ],
                    decoration: InputDecoration(
                      isDense: true,
                      prefixText: '$_baseCurrency ',
                    ),
                    onChanged: (_) => setState(() {}),
                  ),
                ),
              ],
            ),
          ),
        Text(
          'Entered ${Money.format(entered, code: _baseCurrency)} of '
          '${Money.format(total, code: _baseCurrency)}',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: balanced
                    ? Theme.of(context).colorScheme.onSurfaceVariant
                    : Theme.of(context).colorScheme.error,
              ),
        ),
      ],
    );
  }

  List<Widget> _participantRows() {
    final entryMinor = _foreign ? _currencyMinor : _baseMinor;
    final totalCents = Money.tryParseToCents(_amount.text, minorUnits: entryMinor) ?? 0;
    final equalShare =
        _participants.isEmpty ? 0 : totalCents ~/ _participants.length;
    return [
      for (final m in _members)
        _ParticipantRow(
          label: _label(m),
          selected: _participants.contains(m.userId),
          mode: _splitMode,
          controller: _inputFor(m.userId),
          currency: _foreign ? _currency : _baseCurrency,
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
    required this.currency,
    required this.equalShareCents,
    required this.onToggle,
    required this.onInputChanged,
  });

  final String label;
  final bool selected;
  final SplitMode mode;
  final TextEditingController controller;
  final String currency;
  final int equalShareCents;
  final ValueChanged<bool> onToggle;
  final VoidCallback onInputChanged;

  @override
  Widget build(BuildContext context) {
    Widget? trailing;
    if (selected) {
      switch (mode) {
        case SplitMode.equal:
          trailing = Text(Money.format(equalShareCents, code: currency));
        case SplitMode.exact:
          trailing = _input(suffix: currency);
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
