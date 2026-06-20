# Splittr

A cross-platform **expense sharing app** (a Splitwise clone) built with
**Flutter** — one codebase that compiles to native apps for **iOS, Android,
Windows, Linux and macOS**, plus the web.

## Why Flutter?

The brief was native apps for iOS, Android, Windows and Linux from a single
project. Flutter is the strongest fit:

| Option | iOS | Android | Windows | Linux | Notes |
|---|---|---|---|---|---|
| **Flutter** | ✅ | ✅ | ✅ | ✅ | Single codebase, compiles to native ARM/x64, one UI toolkit everywhere |
| React Native | ✅ | ✅ | ⚠️ | ⚠️ | Desktop only via community forks |
| .NET MAUI | ✅ | ✅ | ✅ | ❌ | No official Linux target |
| Electron / Tauri | ❌ | ❌ | ✅ | ✅ | Desktop only |

Flutter is the only mainstream toolkit that covers all four required targets
natively from one codebase.

## Core features

- **Groups** — create groups for trips, households, etc. with an emoji,
  currency and members.
- **Friends** — track one-to-one balances across every shared expense.
- **Add expenses** with four split strategies:
  - **Equally** — even split, remainder cents distributed fairly.
  - **Exact amounts** — enter what each person owes.
  - **Percentages** — split by percentage (validated to 100%).
  - **Shares** — split by weighted shares (e.g. 2:1:1).
- **Categories** for each expense (food, travel, utilities, …).
- **Balances** — per-member balances and an overall "you owe / you are owed".
- **Settle up** — record payments, with **debt simplification** that minimises
  the number of payments needed to settle a group.
- **Activity feed** — a chronological log of expenses, payments and changes.
- **Multi-currency** groups.
- **Offline-first** local persistence — no backend required; your data lives on
  the device.

## Architecture

The project follows a layered, testable architecture (a Flutter best practice):

```
lib/
├── core/                     Cross-cutting helpers (money, theme, categories)
├── domain/                   Pure Dart — no Flutter imports, fully unit-tested
│   ├── models/               Immutable entities (User, Group, Expense, …)
│   └── services/             Business logic
│       ├── split_calculator.dart    The four split strategies
│       ├── balance_calculator.dart  Net + pairwise balances
│       └── debt_simplifier.dart     Minimal-cash-flow settling
├── data/                     Persistence (JSON document store behind an interface)
├── state/                    Riverpod providers + StateNotifier controller
└── ui/                       Screens and reusable widgets
```

Key decisions:

- **Money is integer cents everywhere.** No floating-point money bugs; splits
  always reconcile to the penny via the largest-remainder method.
- **The domain layer has zero Flutter dependencies,** so all the important logic
  is tested with plain `dart test`.
- **State is a single immutable `AppState`** mutated through one controller, so
  every change is explicit and serialization is one method call.
- **Persistence is behind an interface** (`Persistence`) — a JSON file on native
  platforms, in-memory on web, easily swapped for SQLite or a REST API.
- **Riverpod** for compile-safe, decoupled, testable state management.
- **Responsive shell** — a bottom navigation bar on phones, a navigation rail on
  tablet/desktop widths.

## Running

```bash
flutter pub get

# Run on a connected device / desktop / browser
flutter run                       # auto-selects a device
flutter run -d linux              # Linux desktop
flutter run -d windows            # Windows desktop
flutter run -d chrome             # Web
flutter run -d <ios|android id>   # Mobile
```

## Building

```bash
flutter build apk          # Android
flutter build ios          # iOS (on macOS)
flutter build linux        # Linux
flutter build windows      # Windows
flutter build macos        # macOS
flutter build web          # Web
```

> Desktop builds need that platform's native toolchain installed
> (e.g. clang + GTK for Linux, Visual Studio for Windows, Xcode for Apple).

## Testing & analysis

```bash
flutter test               # 32 unit/integration tests
flutter analyze            # strict static analysis (see analysis_options.yaml)
```

The test suite covers the split strategies, balance and pairwise-debt
calculation, debt simplification, model serialization, and the full
controller flow (create group → add expense → settle up → persist & reload).
