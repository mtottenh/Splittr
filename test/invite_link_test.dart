import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/core/invite_link.dart';

void main() {
  group('InviteLink', () {
    test('round-trips token bytes through a shareable link', () {
      final token = Uint8List.fromList(List.generate(40, (i) => (i * 7) % 256));
      final link = InviteLink.encode(token);
      expect(link, startsWith(InviteLink.base));
      expect(link, isNot(contains('=')), reason: 'links are padding-free');
      expect(InviteLink.decode(link), equals(token));
    });

    test('decodes a bare token, a custom scheme, and tolerates trimming', () {
      final token = Uint8List.fromList([1, 2, 3, 4, 5]);
      final bare = InviteLink.encode(token).substring(InviteLink.base.length);
      expect(InviteLink.decode(bare), equals(token));
      expect(InviteLink.decode('splittr://invite/$bare'), equals(token));
      expect(InviteLink.decode('  ${InviteLink.encode(token)}  '), equals(token));
    });

    test('ignores a trailing query or fragment', () {
      final token = Uint8List.fromList([9, 8, 7, 6]);
      final link = InviteLink.encode(token);
      expect(InviteLink.decode('$link?ref=sms'), equals(token));
      expect(InviteLink.decode('$link#section'), equals(token));
    });

    test('rejects empty, truncated or malformed input', () {
      expect(InviteLink.decode(''), isNull);
      expect(InviteLink.decode('   '), isNull);
      expect(InviteLink.decode(InviteLink.base), isNull); // no token segment
      expect(InviteLink.decode('not valid base64 @@@'), isNull);
    });
  });
}
