import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../src/rust/dto.dart';
import '../../state/providers.dart';
import '../root_access.dart';

/// Lists the devices authorized for this identity, with revoke (#16/ADR-0005).
class DevicesScreen extends ConsumerWidget {
  const DevicesScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final devices = ref.watch(deviceListProvider);
    return Scaffold(
      appBar: AppBar(title: const Text('Linked devices')),
      body: devices.when(
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('Error: $e')),
        data: (list) => ListView(
          children: [
            const Padding(
              padding: EdgeInsets.all(16),
              child: Text(
                'Each device signs with its own key under your identity. '
                'Revoke a device to stop it making further changes.',
              ),
            ),
            for (final d in list) _DeviceTile(device: d),
          ],
        ),
      ),
    );
  }
}

class _DeviceTile extends ConsumerWidget {
  const _DeviceTile({required this.device});
  final DeviceViewDto device;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final short =
        '${device.device.substring(0, 8)}…${device.device.substring(56)}';
    final subtitle = [
      'site ${device.site}',
      if (device.revoked) 'revoked',
      if (device.thisDevice) 'this device',
    ].join(' · ');
    return ListTile(
      leading: Icon(device.thisDevice ? Icons.smartphone : Icons.devices_other),
      title: Text(short),
      subtitle: Text(subtitle),
      trailing: (device.thisDevice || device.revoked)
          ? null
          : TextButton(
              onPressed: () => _revoke(context, ref),
              child: const Text('Revoke'),
            ),
    );
  }

  /// Revoking signs a root certificate, so unlock the root first (PIN-prompted
  /// when app lock is on, #34).
  Future<void> _revoke(BuildContext context, WidgetRef ref) async {
    final rootSeed = await obtainRootSeed(context, ref);
    if (rootSeed == null) return; // cancelled
    await ref
        .read(appProvider.notifier)
        .revokeDevice(device.device, rootSeed: rootSeed);
  }
}
