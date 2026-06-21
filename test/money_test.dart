import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/core/money.dart';

void main() {
  group('Money.tryParseToCents', () {
    test('defaults to 2 minor units', () {
      expect(Money.tryParseToCents('12.50'), 1250);
      expect(Money.tryParseToCents('1,234.5'), 123450);
      expect(Money.tryParseToCents('  0.05 '), 5);
    });

    test('honours the currency minor units', () {
      // JPY (0 decimals): "1000" yen is 1000 minor units, not 100000.
      expect(Money.tryParseToCents('1000', minorUnits: 0), 1000);
      // BHD (3 decimals): "1.234" is 1234 minor units.
      expect(Money.tryParseToCents('1.234', minorUnits: 3), 1234);
    });

    test('rejects empty, negative or non-numeric input', () {
      expect(Money.tryParseToCents(''), isNull);
      expect(Money.tryParseToCents('  '), isNull);
      expect(Money.tryParseToCents('-5'), isNull);
      expect(Money.tryParseToCents('abc'), isNull);
    });

    test('toMajor inverts the minor-unit scale', () {
      expect(Money.toMajor(1000, minorUnits: 0), 1000);
      expect(Money.toMajor(1250), 12.5);
      expect(Money.toMajor(1234, minorUnits: 3), 1.234);
    });
  });
}
