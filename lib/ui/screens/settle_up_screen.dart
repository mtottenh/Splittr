import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/money.dart';
import '../../src/rust/dto.dart';
import '../../state/providers.dart';

/// Record a payment between two members to pay down a debt. Pre-fills from the
/// group's suggested settle-up payments.
class SettleUpScreen extends ConsumerStatefulWidget {
  const SettleUpScreen({super.key, required this.group});
  final GroupDetailDto group;

  @override
  ConsumerState<SettleUpScreen> createState() => _SettleUpScreenState();
}

class _SettleUpScreenState extends ConsumerState<SettleUpScreen> {
  final _amount = TextEditingController();
  String? _fromId;
  String? _toId;

  @override
  void initState() {
    super.initState();
    _fromId = ref.read(appProvider).requireValue.myUserId;
  }

  @override
  void dispose() {
    _amount.dispose();
    super.dispose();
  }

  void _applySuggestion(TransferDto t) {
    setState(() {
      _fromId = t.from;
      _toId = t.to;
      _amount.text = Money.toMajor(t.amountCents).toStringAsFixed(2);
    });
  }

  Future<void> _record() async {
    final cents = Money.tryParseToCents(_amount.text);
    if (_fromId == null || _toId == null || _fromId == _toId) {
      _error('Pick two different people.');
      return;
    }
    if (cents == null || cents <= 0) {
      _error('Enter a valid amount.');
      return;
    }
    await ref.read(appProvider.notifier).recordSettlement(
          groupId: widget.group.id,
          from: _fromId!,
          to: _toId!,
          amountCents: cents,
        );
    if (mounted) Navigator.of(context).pop();
  }

  void _error(String message) {
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(message)));
  }

  String _name(String userId) => widget.group.members
      .firstWhere(
        (m) => m.userId == userId,
        orElse: () => MemberBalanceDto(userId: userId, name: userId, netCents: 0),
      )
      .name;

  @override
  Widget build(BuildContext context) {
    final group = widget.group;
    return Scaffold(
      appBar: AppBar(title: const Text('Settle up')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          if (group.settleUp.isNotEmpty) ...[
            Text('Suggested', style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 8),
            for (final t in group.settleUp)
              Card(
                child: ListTile(
                  title: Text('${_name(t.from)} → ${_name(t.to)}'),
                  trailing: Text(
                    Money.format(t.amountCents),
                    style: const TextStyle(fontWeight: FontWeight.bold),
                  ),
                  onTap: () => _applySuggestion(t),
                ),
              ),
            const Divider(height: 32),
          ],
          Text('Record a payment',
              style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 16),
          DropdownButtonFormField<String>(
            initialValue: _fromId,
            decoration: const InputDecoration(labelText: 'From'),
            items: [
              for (final m in group.members)
                DropdownMenuItem(value: m.userId, child: Text(_name(m.userId))),
            ],
            onChanged: (v) => setState(() => _fromId = v),
          ),
          const SizedBox(height: 12),
          DropdownButtonFormField<String>(
            initialValue: _toId,
            decoration: const InputDecoration(labelText: 'To'),
            items: [
              for (final m in group.members)
                DropdownMenuItem(value: m.userId, child: Text(_name(m.userId))),
            ],
            onChanged: (v) => setState(() => _toId = v),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _amount,
            keyboardType: const TextInputType.numberWithOptions(decimal: true),
            inputFormatters: [
              FilteringTextInputFormatter.allow(RegExp(r'[0-9.,]')),
            ],
            decoration: const InputDecoration(
              labelText: 'Amount',
              prefixText: 'USD ',
            ),
          ),
          const SizedBox(height: 24),
          FilledButton.icon(
            onPressed: _record,
            icon: const Icon(Icons.check),
            label: const Text('Record payment'),
          ),
        ],
      ),
    );
  }
}
