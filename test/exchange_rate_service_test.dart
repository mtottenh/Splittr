import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:splittr/services/exchange_rate_service.dart';

/// A canned HTTP client so the service can be tested without network access.
class _FakeClient extends http.BaseClient {
  _FakeClient(this._handler);
  final http.Response Function(http.BaseRequest) _handler;
  int calls = 0;

  @override
  Future<http.StreamedResponse> send(http.BaseRequest request) async {
    calls++;
    final res = _handler(request);
    return http.StreamedResponse(
      Stream.value(utf8.encode(res.body)),
      res.statusCode,
    );
  }
}

void main() {
  test('parses the rate, snaps to the date, and caches', () async {
    http.BaseRequest? seen;
    final client = _FakeClient((req) {
      seen = req;
      return http.Response('{"rates":{"USD":1.08}}', 200);
    });
    final svc = HttpExchangeRateService(client: client);

    final rate = await svc.rate(
      base: 'EUR',
      quote: 'USD',
      on: DateTime.utc(2026, 6, 1),
    );
    expect(rate, 1.08);
    expect(seen!.url.path, '/2026-06-01');
    expect(seen!.url.queryParameters, {'from': 'EUR', 'to': 'USD'});

    // Second identical call is served from cache (no new request).
    await svc.rate(base: 'EUR', quote: 'USD', on: DateTime.utc(2026, 6, 1));
    expect(client.calls, 1);
  });

  test('same base and quote is a no-op (rate 1, no request)', () async {
    final client = _FakeClient((_) => http.Response('{}', 200));
    final svc = HttpExchangeRateService(client: client);
    expect(await svc.rate(base: 'USD', quote: 'USD'), 1.0);
    expect(client.calls, 0);
  });

  test('returns null on a server error (caller falls back to manual)', () async {
    final client = _FakeClient((_) => http.Response('nope', 500));
    final svc = HttpExchangeRateService(client: client);
    expect(await svc.rate(base: 'EUR', quote: 'GBP'), isNull);
  });

  test('returns null when the quote is missing from the response', () async {
    final client = _FakeClient((_) => http.Response('{"rates":{}}', 200));
    final svc = HttpExchangeRateService(client: client);
    expect(await svc.rate(base: 'EUR', quote: 'JPY'), isNull);
  });
}
