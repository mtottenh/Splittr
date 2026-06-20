import 'package:flutter/material.dart';

import '../../core/money.dart';
import '../../core/theme.dart';

/// A two-line balance label: a coloured amount over a "you owe / owes you /
/// settled up" caption. Used in lists and headers throughout the app.
class BalanceLabel extends StatelessWidget {
  const BalanceLabel({
    super.key,
    required this.netCents,
    required this.currencyCode,
    this.youArePositive = 'owes you',
    this.youAreNegative = 'you owe',
    this.settledText = 'settled up',
    this.alignEnd = true,
  });

  /// Positive = the subject is owed money; negative = the subject owes.
  final int netCents;
  final String currencyCode;
  final String youArePositive;
  final String youAreNegative;
  final String settledText;
  final bool alignEnd;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final color = AppTheme.balanceColor(netCents, theme.colorScheme);

    if (netCents == 0) {
      return Text(
        settledText,
        style: theme.textTheme.bodyMedium
            ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
      );
    }

    return Column(
      crossAxisAlignment:
          alignEnd ? CrossAxisAlignment.end : CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          netCents > 0 ? youArePositive : youAreNegative,
          style: theme.textTheme.labelSmall?.copyWith(color: color),
        ),
        Text(
          Money.formatAbs(netCents, code: currencyCode),
          style: theme.textTheme.titleMedium
              ?.copyWith(color: color, fontWeight: FontWeight.bold),
        ),
      ],
    );
  }
}
