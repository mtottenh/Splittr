import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/state/app_lock.dart';

void main() {
  test('hashPin is deterministic and salt-dependent', () {
    expect(hashPin('1234', 'saltA'), hashPin('1234', 'saltA'));
    // Same PIN, different salt → different hash (resists precomputation).
    expect(hashPin('1234', 'saltA'), isNot(hashPin('1234', 'saltB')));
    // Different PIN, same salt → different hash.
    expect(hashPin('1234', 'saltA'), isNot(hashPin('9999', 'saltA')));
  });

  test('hashPin returns a 64-char hex SHA-256 digest', () {
    final h = hashPin('0000', 'salt');
    expect(h, hasLength(64));
    expect(RegExp(r'^[0-9a-f]{64}$').hasMatch(h), isTrue);
  });
}
