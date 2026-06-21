import 'package:intl/intl.dart';

/// Helpers for working with monetary amounts.
///
/// Every amount in the domain is stored as an integer number of **minor units**
/// (cents). Integer arithmetic avoids the rounding errors that plague
/// floating-point money and guarantees that splits always reconcile to the
/// penny.
class Money {
  const Money._();

  /// Parses user input such as `"12.50"` or `"1,234.5"` into integer minor
  /// units. [minorUnits] is the currency's decimal places (engine
  /// `currencyMinorUnits`): 2 for USD, 0 for JPY, 3 for BHD — so the scale is
  /// never hardcoded. Defaults to 2 for callers in a known-2-decimal context.
  ///
  /// Returns `null` when the text cannot be interpreted as a non-negative amount.
  static int? tryParseToCents(String input, {int minorUnits = 2}) {
    final cleaned = input.replaceAll(',', '').trim();
    if (cleaned.isEmpty) return null;
    final value = double.tryParse(cleaned);
    if (value == null || value.isNaN || value.isInfinite) return null;
    if (value < 0) return null;
    return (value * _pow10(minorUnits)).round();
  }

  /// Converts integer minor units back into a major-unit double (e.g. dollars).
  static double toMajor(int cents, {int minorUnits = 2}) =>
      cents / _pow10(minorUnits);

  static int _pow10(int n) {
    var r = 1;
    for (var i = 0; i < n; i++) {
      r *= 10;
    }
    return r;
  }

  /// Formats [cents] using the currency [code]'s symbol and locale.
  static String format(int cents, {String code = 'USD', String? locale}) {
    final format = NumberFormat.simpleCurrency(
      locale: locale,
      name: code,
    );
    return format.format(cents / 100.0);
  }

  /// Like [format] but never shows a leading minus sign — useful when the UI
  /// already conveys direction ("you owe" / "owes you").
  static String formatAbs(int cents, {String code = 'USD', String? locale}) =>
      format(cents.abs(), code: code, locale: locale);
}
