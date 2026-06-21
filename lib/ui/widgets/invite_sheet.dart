import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../../core/invite_link.dart';
import '../../state/providers.dart';
import '../root_access.dart';

/// Generate a friend invite and present it as a link + QR (#7). Signing needs
/// the identity (root) key, so the user is PIN-prompted when app lock is on.
Future<void> showFriendInvite(BuildContext context, WidgetRef ref) async {
  await _present(
    context,
    ref,
    title: 'Invite a friend',
    subtitle: 'Share this link or QR. They become a friend once you both add '
        'each other.',
    create: (rootSeed) =>
        ref.read(appProvider.notifier).createFriendInvite(rootSeed: rootSeed),
  );
}

/// Generate an invite to join [groupName] and present it (#7).
Future<void> showGroupInvite(
  BuildContext context,
  WidgetRef ref, {
  required String groupId,
  required String groupName,
}) async {
  await _present(
    context,
    ref,
    title: 'Invite to $groupName',
    subtitle: 'Share this link or QR so they can join the group.',
    create: (rootSeed) => ref
        .read(appProvider.notifier)
        .createGroupInvite(groupId, rootSeed: rootSeed),
  );
}

Future<void> _present(
  BuildContext context,
  WidgetRef ref, {
  required String title,
  required String subtitle,
  required Future<List<int>> Function(List<int> rootSeed) create,
}) async {
  final rootSeed = await obtainRootSeed(context, ref);
  if (rootSeed == null || !context.mounted) return; // cancelled
  final List<int> token;
  try {
    token = await create(rootSeed);
  } catch (e) {
    if (context.mounted) {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text('Could not create invite: $e')));
    }
    return;
  }
  if (!context.mounted) return;
  await showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    showDragHandle: true,
    builder: (context) =>
        InviteSheet(title: title, subtitle: subtitle, link: InviteLink.encode(token)),
  );
}

/// The presentation body for a generated invite: a QR code, the shareable link,
/// and a copy action. Stateless so it can be unit-tested with a fixed link.
class InviteSheet extends StatelessWidget {
  const InviteSheet({
    super.key,
    required this.title,
    required this.subtitle,
    required this.link,
  });

  final String title;
  final String subtitle;
  final String link;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(24, 0, 24, 24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(title, style: theme.textTheme.titleLarge),
            const SizedBox(height: 8),
            Text(subtitle, style: theme.textTheme.bodyMedium),
            const SizedBox(height: 20),
            Center(
              child: Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: Colors.white,
                  borderRadius: BorderRadius.circular(12),
                ),
                child: QrImageView(
                  data: link,
                  version: QrVersions.auto,
                  size: 220,
                  // Keep the QR readable regardless of the surrounding theme.
                  backgroundColor: Colors.white,
                ),
              ),
            ),
            const SizedBox(height: 20),
            SelectableText(
              link,
              style: theme.textTheme.bodySmall
                  ?.copyWith(fontFamily: 'monospace', color: theme.hintColor),
              maxLines: 2,
            ),
            const SizedBox(height: 12),
            FilledButton.icon(
              onPressed: () => _copy(context),
              icon: const Icon(Icons.copy),
              label: const Text('Copy link'),
            ),
            const SizedBox(height: 4),
            Text(
              'Expires in 7 days.',
              textAlign: TextAlign.center,
              style: theme.textTheme.labelSmall?.copyWith(color: theme.hintColor),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _copy(BuildContext context) async {
    await Clipboard.setData(ClipboardData(text: link));
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Invite link copied')),
      );
    }
  }
}
