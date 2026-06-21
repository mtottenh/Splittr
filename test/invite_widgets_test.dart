import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:splittr/core/invite_link.dart';
import 'package:splittr/src/rust/dto.dart';
import 'package:splittr/state/app_data.dart';
import 'package:splittr/state/providers.dart';
import 'package:splittr/ui/widgets/accept_invite_dialog.dart';
import 'package:splittr/ui/widgets/invite_sheet.dart';

/// A facade stand-in: the dialog only touches [previewInvite] and [addFriend],
/// so we record those and never reach the real engine.
class _FakeAppNotifier extends AppNotifier {
  _FakeAppNotifier(this._preview);
  final InviteDto? _preview;
  final List<String> added = [];

  @override
  Future<AppData> build() async => const AppData(
        myUserId: 'id:me',
        myName: 'Me',
        overallNetCents: 0,
        groups: [],
        friends: [],
        activity: [],
      );

  @override
  Future<InviteDto?> previewInvite(List<int> invite) async => _preview;

  @override
  Future<void> addFriend(String userId) async => added.add(userId);
}

InviteDto _dto({
  String inviter = 'id:abababababababababababababababababababababababababababababababab',
  String context = 'friend',
  bool valid = true,
}) =>
    InviteDto(
      inviter: inviter,
      context: context,
      expiryMs: BigInt.from(9999999999999),
      valid: valid,
    );

Future<void> _openAccept(WidgetTester tester, _FakeAppNotifier fake) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [appProvider.overrideWith(() => fake)],
      child: MaterialApp(
        home: Consumer(
          builder: (context, ref, _) => Scaffold(
            body: Center(
              child: TextButton(
                onPressed: () => showAcceptInviteDialog(context, ref),
                child: const Text('open'),
              ),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
}

Future<void> _checkLink(WidgetTester tester, String link) async {
  await tester.enterText(find.byType(TextField), link);
  await tester.tap(find.widgetWithText(FilledButton, 'Check'));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('invite sheet renders the link, a QR and copies it', (tester) async {
    // Mock the clipboard platform channel so Copy doesn't throw.
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async => null,
    );
    const link = 'https://splittr.app/i/AAECAwQF';

    await tester.pumpWidget(const MaterialApp(
      home: Scaffold(
        body: InviteSheet(
          title: 'Invite a friend',
          subtitle: 'Share this link.',
          link: link,
        ),
      ),
    ));

    expect(find.text('Invite a friend'), findsOneWidget);
    expect(find.text(link), findsOneWidget);
    expect(find.byType(QrImageView), findsOneWidget);

    await tester.tap(find.widgetWithText(FilledButton, 'Copy link'));
    await tester.pump();
    expect(find.text('Invite link copied'), findsOneWidget);
  });

  testWidgets('accept dialog previews a friend invite and adds the friend',
      (tester) async {
    final fake = _FakeAppNotifier(_dto());
    await _openAccept(tester, fake);
    await _checkLink(tester, InviteLink.encode([1, 2, 3, 4, 5]));

    expect(find.text('Friend invite'), findsOneWidget);
    expect(find.textContaining('abababab'), findsOneWidget);

    await tester.tap(find.widgetWithText(FilledButton, 'Add friend'));
    await tester.pumpAndSettle();
    expect(fake.added, [
      'id:abababababababababababababababababababababababababababababababab'
    ]);
  });

  testWidgets('accept dialog rejects an invalid/expired invite', (tester) async {
    final fake = _FakeAppNotifier(_dto(valid: false));
    await _openAccept(tester, fake);
    await _checkLink(tester, InviteLink.encode([1, 2, 3, 4, 5]));

    expect(find.text('This invite is invalid or has expired.'), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Add friend'), findsNothing);
    expect(fake.added, isEmpty);
  });

  testWidgets('accept dialog previews a group invite without a join action',
      (tester) async {
    final fake = _FakeAppNotifier(_dto(context: 'group:g1'));
    await _openAccept(tester, fake);
    await _checkLink(tester, InviteLink.encode([1, 2, 3, 4, 5]));

    expect(find.text('Group invite'), findsOneWidget);
    expect(find.textContaining('needs the inviter'), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Add friend'), findsNothing);
    expect(find.widgetWithText(FilledButton, 'Check'), findsOneWidget);
  });

  testWidgets('accept dialog reports an unparseable link', (tester) async {
    final fake = _FakeAppNotifier(null);
    await _openAccept(tester, fake);
    await _checkLink(tester, 'not a real link @@@');

    expect(find.text("That doesn't look like a valid invite link."),
        findsOneWidget);
    expect(fake.added, isEmpty);
  });
}
