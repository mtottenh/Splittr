import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../state/providers.dart';
import 'screens/account_screen.dart';
import 'screens/activity_screen.dart';
import 'screens/add_expense_screen.dart';
import 'screens/friends_screen.dart';
import 'screens/groups_screen.dart';

/// The app's top-level navigation. Adapts between a bottom navigation bar on
/// narrow (phone) layouts and a navigation rail on wide (tablet/desktop)
/// layouts — a Flutter best practice for spanning mobile and desktop from one
/// widget tree.
class RootShell extends ConsumerStatefulWidget {
  const RootShell({super.key});

  @override
  ConsumerState<RootShell> createState() => _RootShellState();
}

class _RootShellState extends ConsumerState<RootShell> {
  int _index = 0;

  static const _destinations = <_Destination>[
    _Destination('Groups', Icons.groups_outlined, Icons.groups),
    _Destination('Friends', Icons.person_outline, Icons.person),
    _Destination('Activity', Icons.history_outlined, Icons.history),
    _Destination('Account', Icons.settings_outlined, Icons.settings),
  ];

  static const _pages = <Widget>[
    GroupsScreen(),
    FriendsScreen(),
    ActivityScreen(),
    AccountScreen(),
  ];

  @override
  Widget build(BuildContext context) {
    final app = ref.watch(appProvider);

    return app.when(
      loading: () => const _Splash(),
      error: (error, _) => _ErrorScreen(error: error),
      data: (_) => _buildShell(context),
    );
  }

  Widget _buildShell(BuildContext context) {
    final isWide = MediaQuery.sizeOf(context).width >= 720;
    final body = _pages[_index];

    // Only the first two tabs are tied to a primary "add expense" action.
    final showFab = _index <= 1;
    final fab = showFab
        ? FloatingActionButton.extended(
            onPressed: () => _openAddExpense(context),
            icon: const Icon(Icons.add),
            label: const Text('Add expense'),
          )
        : null;

    if (isWide) {
      return Scaffold(
        body: Row(
          children: [
            NavigationRail(
              selectedIndex: _index,
              onDestinationSelected: (i) => setState(() => _index = i),
              labelType: NavigationRailLabelType.all,
              leading: fab == null
                  ? null
                  : Padding(
                      padding: const EdgeInsets.symmetric(vertical: 8),
                      child: FloatingActionButton(
                        onPressed: () => _openAddExpense(context),
                        elevation: 0,
                        child: const Icon(Icons.add),
                      ),
                    ),
              destinations: [
                for (final d in _destinations)
                  NavigationRailDestination(
                    icon: Icon(d.icon),
                    selectedIcon: Icon(d.selectedIcon),
                    label: Text(d.label),
                  ),
              ],
            ),
            const VerticalDivider(width: 1),
            Expanded(child: body),
          ],
        ),
      );
    }

    return Scaffold(
      body: body,
      floatingActionButton: fab,
      bottomNavigationBar: NavigationBar(
        selectedIndex: _index,
        onDestinationSelected: (i) => setState(() => _index = i),
        destinations: [
          for (final d in _destinations)
            NavigationDestination(
              icon: Icon(d.icon),
              selectedIcon: Icon(d.selectedIcon),
              label: d.label,
            ),
        ],
      ),
    );
  }

  /// Every expense belongs to a group, so first resolve which group to add to,
  /// then load its detail (members) before opening the editor.
  Future<void> _openAddExpense(BuildContext context) async {
    final groups = ref.read(appProvider).requireValue.groups;
    if (groups.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Create a group first.')),
      );
      return;
    }

    String groupId = groups.first.id;
    if (groups.length > 1) {
      final picked = await showModalBottomSheet<String>(
        context: context,
        builder: (context) => SafeArea(
          child: ListView(
            shrinkWrap: true,
            children: [
              for (final g in groups)
                ListTile(
                  leading: const Icon(Icons.groups),
                  title: Text(g.name),
                  onTap: () => Navigator.of(context).pop(g.id),
                ),
            ],
          ),
        ),
      );
      if (picked == null) return;
      groupId = picked;
    }

    final detail = await ref.read(groupDetailProvider(groupId).future);
    if (detail == null || !context.mounted) return;
    await Navigator.of(context).push(
      MaterialPageRoute<void>(
        builder: (_) => AddExpenseScreen(group: detail),
        fullscreenDialog: true,
      ),
    );
  }
}

class _Destination {
  const _Destination(this.label, this.icon, this.selectedIcon);
  final String label;
  final IconData icon;
  final IconData selectedIcon;
}

class _Splash extends StatelessWidget {
  const _Splash();

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.call_split,
                size: 56, color: Theme.of(context).colorScheme.primary),
            const SizedBox(height: 16),
            const Text('Splittr'),
            const SizedBox(height: 24),
            const CircularProgressIndicator(),
          ],
        ),
      ),
    );
  }
}

class _ErrorScreen extends StatelessWidget {
  const _ErrorScreen({required this.error});
  final Object error;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(child: Text('Failed to start: $error')),
    );
  }
}
