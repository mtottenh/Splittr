// Widget tests for first-run onboarding (#34): the create flow shows the
// recovery phrase behind a "written it down" gate, and the restore flow feeds
// the entered phrase to the bootstrap. The identity bootstrap is faked so these
// run without the FFI or a keystore.

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:splittr/state/engine.dart';
import 'package:splittr/ui/screens/onboarding_screen.dart';

class _FakeBootstrap implements IdentityBootstrap {
  String? restoredPhrase;

  @override
  Future<String> createNew() async => List.filled(24, 'abandon').join(' ');

  @override
  Future<bool> restore(String phrase) async {
    restoredPhrase = phrase;
    return true;
  }
}

Widget _wrap(_FakeBootstrap fake) => ProviderScope(
      overrides: [
        identityBootstrapProvider.overrideWith((ref) async => fake),
        identityEstablishedProvider.overrideWith((ref) async => false),
      ],
      child: const MaterialApp(home: OnboardingScreen()),
    );

void main() {
  testWidgets('offers create and restore', (tester) async {
    await tester.pumpWidget(_wrap(_FakeBootstrap()));
    expect(find.text('Create a new identity'), findsOneWidget);
    expect(find.text('Restore from recovery phrase'), findsOneWidget);
  });

  testWidgets('create reveals the phrase and gates on acknowledgement',
      (tester) async {
    await tester.pumpWidget(_wrap(_FakeBootstrap()));
    await tester.tap(find.text('Create a new identity'));
    await tester.pumpAndSettle();

    // The phrase dialog is shown and Continue is disabled until acknowledged.
    expect(find.text('Your recovery phrase'), findsOneWidget);
    FilledButton continueBtn() =>
        tester.widget<FilledButton>(find.widgetWithText(FilledButton, 'Continue'));
    expect(continueBtn().onPressed, isNull);

    await tester.tap(find.byType(Checkbox));
    await tester.pumpAndSettle();
    expect(continueBtn().onPressed, isNotNull);

    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();
    expect(find.text('Your recovery phrase'), findsNothing);
  });

  testWidgets('restore passes the entered phrase to the bootstrap',
      (tester) async {
    final fake = _FakeBootstrap();
    await tester.pumpWidget(_wrap(fake));
    await tester.tap(find.text('Restore from recovery phrase'));
    await tester.pumpAndSettle();

    await tester.enterText(find.byType(TextField), 'one two three');
    await tester.tap(find.text('Restore'));
    await tester.pumpAndSettle();

    expect(fake.restoredPhrase, 'one two three');
  });
}
