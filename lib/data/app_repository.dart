import 'dart:convert';

import '../state/app_state.dart';
import 'persistence.dart';

/// Loads and saves the [AppState] snapshot through a [Persistence] backend.
///
/// The repository owns serialization; callers work purely with [AppState].
class AppRepository {
  AppRepository(this._persistence);

  final Persistence _persistence;

  Future<AppState?> load() async {
    final raw = await _persistence.read();
    if (raw == null) return null;
    final decoded = jsonDecode(raw) as Map<String, dynamic>;
    return AppState.fromJson(decoded);
  }

  Future<void> save(AppState state) =>
      _persistence.write(jsonEncoder.convert(state.toJson()));
}
