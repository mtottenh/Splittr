import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/invite_link.dart';
import '../../src/rust/dto.dart';
import '../../state/providers.dart';

/// Paste an invite link, verify it, preview who sent it, and accept (#7).
///
/// A friend invite is accepted by declaring friendship toward the inviter (the
/// edge confirms once they declare back). A group invite is preview-only for
/// now: joining requires the inviter to admit you, which needs peering (#9) — so
/// this surfaces the trust preview without a misleading "join" button.
Future<void> showAcceptInviteDialog(
  BuildContext context,
  WidgetRef ref, {
  String? initialLink,
}) {
  return showDialog<void>(
    context: context,
    builder: (context) => _AcceptInviteDialog(initialLink: initialLink),
  );
}

class _AcceptInviteDialog extends ConsumerStatefulWidget {
  const _AcceptInviteDialog({this.initialLink});
  final String? initialLink;

  @override
  ConsumerState<_AcceptInviteDialog> createState() =>
      _AcceptInviteDialogState();
}

class _AcceptInviteDialogState extends ConsumerState<_AcceptInviteDialog> {
  late final TextEditingController _link =
      TextEditingController(text: widget.initialLink ?? '');
  InviteDto? _preview;
  bool _checked = false; // a check has run (distinguishes "no preview yet")
  bool _busy = false;

  @override
  void dispose() {
    _link.dispose();
    super.dispose();
  }

  Future<void> _check() async {
    final bytes = InviteLink.decode(_link.text);
    if (bytes == null) {
      setState(() {
        _preview = null;
        _checked = true;
      });
      return;
    }
    setState(() => _busy = true);
    final dto = await ref.read(appProvider.notifier).previewInvite(bytes);
    if (!mounted) return;
    setState(() {
      _preview = dto;
      _checked = true;
      _busy = false;
    });
  }

  Future<void> _acceptFriend() async {
    final dto = _preview!;
    setState(() => _busy = true);
    await ref.read(appProvider.notifier).addFriend(dto.inviter);
    if (!mounted) return;
    Navigator.of(context).pop();
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('Friend added — confirmed once they add you back.'),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Accept an invite'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          TextField(
            controller: _link,
            autofocus: true,
            minLines: 1,
            maxLines: 3,
            decoration: const InputDecoration(
              labelText: 'Invite link',
              hintText: 'Paste the link you were sent',
            ),
            onChanged: (_) {
              if (_checked) setState(() => _checked = false);
            },
          ),
          if (_busy) ...[
            const SizedBox(height: 16),
            const Center(child: CircularProgressIndicator()),
          ] else if (_checked)
            Padding(
              padding: const EdgeInsets.only(top: 16),
              child: _Preview(dto: _preview),
            ),
        ],
      ),
      actions: _actions(),
    );
  }

  List<Widget> _actions() {
    final cancel = TextButton(
      onPressed: _busy ? null : () => Navigator.of(context).pop(),
      child: const Text('Close'),
    );
    final dto = _preview;
    // A valid friend invite is the one thing we can act on locally.
    if (_checked && dto != null && dto.valid && dto.context == 'friend') {
      return [
        cancel,
        FilledButton(
          onPressed: _busy ? null : _acceptFriend,
          child: const Text('Add friend'),
        ),
      ];
    }
    return [
      cancel,
      FilledButton(
        onPressed: _busy ? null : _check,
        child: const Text('Check'),
      ),
    ];
  }
}

class _Preview extends StatelessWidget {
  const _Preview({required this.dto});
  final InviteDto? dto;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    if (dto == null) {
      return Text(
        "That doesn't look like a valid invite link.",
        style: theme.textTheme.bodyMedium?.copyWith(color: theme.colorScheme.error),
      );
    }
    if (!dto!.valid) {
      return Text(
        'This invite is invalid or has expired.',
        style: theme.textTheme.bodyMedium?.copyWith(color: theme.colorScheme.error),
      );
    }
    final isFriend = dto!.context == 'friend';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Icon(isFriend ? Icons.person_add : Icons.groups,
                color: theme.colorScheme.primary),
            const SizedBox(width: 8),
            Text(isFriend ? 'Friend invite' : 'Group invite',
                style: theme.textTheme.titleMedium),
          ],
        ),
        const SizedBox(height: 8),
        Text('From: ${_fingerprint(dto!.inviter)}',
            style: theme.textTheme.bodyMedium),
        if (!isFriend) ...[
          const SizedBox(height: 8),
          Text(
            'Joining a group needs the inviter to be online (coming with '
            'sync). You can verify who sent it above for now.',
            style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor),
          ),
        ],
      ],
    );
  }

  /// A short, comparable fingerprint of the inviter's identity for the trust
  /// preview. [userId] is `id:<hex>`; show the head and tail of the key.
  static String _fingerprint(String userId) {
    final hex = userId.startsWith('id:') ? userId.substring(3) : userId;
    if (hex.length <= 16) return hex;
    return '${hex.substring(0, 8)}…${hex.substring(hex.length - 8)}';
  }
}
