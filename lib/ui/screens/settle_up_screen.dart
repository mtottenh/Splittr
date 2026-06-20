import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/money.dart';
import '../../domain/models/balance.dart';
import '../../state/providers.dart';

/// Record a payment between two people to pay down a debt. Pre-fills from the
/// group's suggested settle-up payments.
class SettleUpScreen extends ConsumerStatefulWidget {
  const SettleUpScreen({super.key, required this.groupId});
  final String groupId;

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
    final state = ref.read(appControllerProvider);
    _fromId = state.currentUserId;
  }

  @override
  void dispose() {
    _amount.dispose();
    super.dispose();
  }

  void _applySuggestion(DebtEdge edge) {
    setState(() {
      _fromId = edge.fromUserId;
      _toId = edge.toUserId;
      _amount.text = Money.toMajor(edge.amountCents).toStringAsFixed(2);
    });
  }

  Future<void> _record() async {
    final state = ref.read(appControllerProvider);
    final group = state.groupById(widget.groupId)!;
    final cents = Money.tryParseToCents(_amount.text);
    if (_fromId == null || _toId == null || _fromId == _toId) {
      _error('Pick two different people.');
      return;
    }
    if (cents == null || cents <= 0) {
      _error('Enter a valid amount.');
      return;
    }
    await ref.read(appControllerProvider.notifier).addSettlement(
          groupId: widget.groupId,
          fromUserId: _fromId!,
          toUserId: _toId!,
          amountCents: cents,
          currencyCode: group.currencyCode,
        );
    if (mounted) Navigator.of(context).pop();
  }

  void _error(String message) {
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(message)));
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(appControllerProvider);
    final group = state.groupById(widget.groupId)!;
    final suggestions = ref.watch(groupSettleUpProvider(widget.groupId));

    return Scaffold(
      appBar: AppBar(title: const Text('Settle up')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          if (suggestions.isNotEmpty) ...[
            Text('Suggested', style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 8),
            for (final edge in suggestions)
              Card(
                child: ListTile(
                  title: Text(
                    '${state.displayName(edge.fromUserId)} → '
                    '${state.displayName(edge.toUserId)}',
                  ),
                  trailing: Text(
                    Money.format(edge.amountCents, code: group.currencyCode),
                    style: const TextStyle(fontWeight: FontWeight.bold),
                  ),
                  onTap: () => _applySuggestion(edge),
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
              for (final id in group.memberIds)
                DropdownMenuItem(
                    value: id, child: Text(state.displayName(id))),
            ],
            onChanged: (v) => setState(() => _fromId = v),
          ),
          const SizedBox(height: 12),
          DropdownButtonFormField<String>(
            initialValue: _toId,
            decoration: const InputDecoration(labelText: 'To'),
            items: [
              for (final id in group.memberIds)
                DropdownMenuItem(
                    value: id, child: Text(state.displayName(id))),
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
            decoration: InputDecoration(
              labelText: 'Amount',
              prefixText: '${group.currencyCode} ',
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
