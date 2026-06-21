import 'dart:convert';
import 'dart:typed_data';

/// The canonical shareable form of an invite token (#7).
///
/// An invite's *trust* content — signature, expiry, context — is produced and
/// verified entirely in the Rust engine (`create_*_invite` / `verify_invite`).
/// This only wraps the opaque token bytes in a shareable URL (and unwraps a
/// pasted one). It is the single place the link format is defined, so the
/// generate and accept flows always agree.
class InviteLink {
  InviteLink._();

  /// Universal-link host/path. A real deployment hosts the
  /// apple-app-site-association / assetlinks.json here so the OS opens the app
  /// on tap; until OS deep linking lands (the #7 tail) links are shared and
  /// pasted manually, which [decode] also supports.
  static const base = 'https://splittr.app/i/';

  /// Encode opaque token [bytes] as a shareable link (base64url, unpadded).
  static String encode(List<int> bytes) =>
      '$base${base64Url.encode(bytes).replaceAll('=', '')}';

  /// Extract token bytes from a pasted link, a custom-scheme URI, or a bare
  /// base64url token. Returns null when [text] carries no decodable token.
  static Uint8List? decode(String text) {
    var s = text.trim();
    if (s.isEmpty) return null;
    // Take the last path segment of a URL, then drop any query/fragment.
    s = s.split('/').last.split('?').first.split('#').first;
    if (s.isEmpty) return null;
    // Restore base64 padding stripped by [encode] (or by a sharing app).
    final remainder = s.length % 4;
    if (remainder != 0) s = s.padRight(s.length + (4 - remainder), '=');
    try {
      final bytes = base64Url.decode(s);
      return bytes.isEmpty ? null : bytes;
    } catch (_) {
      return null;
    }
  }
}
