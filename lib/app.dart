import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'core/theme.dart';
import 'state/app_lock.dart';
import 'ui/lock_screen.dart';
import 'ui/root_shell.dart';

/// Root widget: wires up theming and the app-lock gate, then hands off to the
/// bootstrapping shell.
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
      home: const _LockGate(),
    );
  }
}

/// Shows the [LockScreen] while the app is locked and re-locks it when the app
/// is backgrounded (#22).
class _LockGate extends ConsumerStatefulWidget {
  const _LockGate();

  @override
  ConsumerState<_LockGate> createState() => _LockGateState();
}

class _LockGateState extends ConsumerState<_LockGate>
    with WidgetsBindingObserver {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.hidden) {
      ref.read(appLockProvider.notifier).lock();
    }
  }

  @override
  Widget build(BuildContext context) {
    final lock = ref.watch(appLockProvider);
    return switch (lock.status) {
      LockStatus.unknown => const _Splash(),
      LockStatus.locked => const LockScreen(),
      LockStatus.unlocked => const RootShell(),
    };
  }
}

class _Splash extends StatelessWidget {
  const _Splash();

  @override
  Widget build(BuildContext context) {
    return const Scaffold(body: Center(child: CircularProgressIndicator()));
  }
}
