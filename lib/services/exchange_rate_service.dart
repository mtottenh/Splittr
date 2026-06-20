import 'dart:convert';

import 'package:http/http.dart' as http;

/// Fetches foreign-exchange rates for multi-currency expenses (#3).
///
/// The conversion *maths* lives in the Rust engine (`convertCurrency`); this is
/// purely the I/O adapter that obtains a rate. It snaps to the expense's date so
/// historical expenses stay stable, caches results, and degrades gracefully
/// offline (callers fall back to a manual rate).
abstract class ExchangeRateService {
  /// The rate to multiply a `base`-currency amount by to get `quote` — i.e.
  /// `quote per 1 base`. Returns null when unavailable (offline / unknown pair).
  Future<double?> rate({
    required String base,
    required String quote,
    DateTime? on,
  });
}

/// [ExchangeRateService] backed by Frankfurter (https://frankfurter.dev) — a
/// free, no-key ECB-data API — with an in-memory day cache.
class HttpExchangeRateService implements ExchangeRateService {
  HttpExchangeRateService({http.Client? client, this.host = 'api.frankfurter.app'})
      : _client = client ?? http.Client();

  final http.Client _client;
  final String host;
  final Map<String, double> _cache = {};

  @override
  Future<double?> rate({
    required String base,
    required String quote,
    DateTime? on,
  }) async {
    if (base == quote) return 1.0;
    final day = _day(on);
    final key = '$day:$base:$quote';
    final cached = _cache[key];
    if (cached != null) return cached;

    try {
      final uri = Uri.https(host, '/$day', {'from': base, 'to': quote});
      final res = await _client.get(uri);
      if (res.statusCode != 200) return null;
      final body = jsonDecode(res.body) as Map<String, dynamic>;
      final rates = body['rates'] as Map<String, dynamic>?;
      final value = rates?[quote];
      if (value is num) {
        final rate = value.toDouble();
        _cache[key] = rate;
        return rate;
      }
      return null;
    } catch (_) {
      return null; // offline / parse error → caller uses a manual rate
    }
  }

  /// Frankfurter snaps to the latest available rate on/after the date; we ask
  /// for the expense's date (or "latest" when none).
  String _day(DateTime? on) {
    if (on == null) return 'latest';
    final d = on.toUtc();
    final mm = d.month.toString().padLeft(2, '0');
    final dd = d.day.toString().padLeft(2, '0');
    return '${d.year}-$mm-$dd';
  }
}
