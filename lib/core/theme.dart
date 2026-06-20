import 'package:flutter/material.dart';

/// Centralised theming so colours, shapes and typography stay consistent and
/// are defined in exactly one place. Material 3 is enabled for modern,
/// platform-adaptive components across mobile and desktop.
class AppTheme {
  const AppTheme._();

  /// Splittr brand seed colour (a Splitwise-like teal/green).
  static const Color seed = Color(0xFF1CC29F);

  static const Color positive = Color(0xFF159F84); // "you are owed"
  static const Color negative = Color(0xFFEF6C57); // "you owe"

  static ThemeData light() => _base(Brightness.light);
  static ThemeData dark() => _base(Brightness.dark);

  static ThemeData _base(Brightness brightness) {
    final scheme = ColorScheme.fromSeed(
      seedColor: seed,
      brightness: brightness,
    );
    return ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      scaffoldBackgroundColor: scheme.surface,
      appBarTheme: AppBarTheme(
        backgroundColor: scheme.surface,
        foregroundColor: scheme.onSurface,
        centerTitle: false,
        elevation: 0,
        scrolledUnderElevation: 1,
      ),
      cardTheme: CardThemeData(
        elevation: 0,
        clipBehavior: Clip.antiAlias,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(16),
          side: BorderSide(color: scheme.outlineVariant),
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
        ),
        filled: true,
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size.fromHeight(48),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
        ),
      ),
      listTileTheme: const ListTileThemeData(
        contentPadding: EdgeInsets.symmetric(horizontal: 16),
      ),
    );
  }

  /// Colour used to render a balance based on its sign.
  static Color balanceColor(int cents, ColorScheme scheme) {
    if (cents > 0) return positive;
    if (cents < 0) return negative;
    return scheme.onSurfaceVariant;
  }
}
