import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'core/theme.dart';
import 'ui/root_shell.dart';

/// Root widget: wires up theming and hands off to the bootstrapping shell.
class SplittrApp extends ConsumerWidget {
  const SplittrApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return MaterialApp(
      title: 'Splittr',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.light(),
      darkTheme: AppTheme.dark(),
      themeMode: ThemeMode.system,
      home: const RootShell(),
    );
  }
}
