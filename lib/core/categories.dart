import 'package:flutter/material.dart';

/// A predefined expense category. Categories are static data, referenced by a
/// stable [id] so renaming a label never breaks stored expenses.
@immutable
class ExpenseCategory {
  const ExpenseCategory(this.id, this.label, this.icon);

  final String id;
  final String label;
  final IconData icon;
}

/// The catalogue of categories offered when adding an expense.
class Categories {
  const Categories._();

  static const List<ExpenseCategory> all = [
    ExpenseCategory('general', 'General', Icons.receipt_long),
    ExpenseCategory('food', 'Food & Drink', Icons.restaurant),
    ExpenseCategory('groceries', 'Groceries', Icons.local_grocery_store),
    ExpenseCategory('transport', 'Transport', Icons.directions_car),
    ExpenseCategory('home', 'Home', Icons.home),
    ExpenseCategory('utilities', 'Utilities', Icons.bolt),
    ExpenseCategory('entertainment', 'Entertainment', Icons.movie),
    ExpenseCategory('travel', 'Travel', Icons.flight),
    ExpenseCategory('shopping', 'Shopping', Icons.shopping_bag),
    ExpenseCategory('health', 'Health', Icons.medical_services),
    ExpenseCategory('gifts', 'Gifts', Icons.card_giftcard),
  ];

  static const ExpenseCategory fallback =
      ExpenseCategory('general', 'General', Icons.receipt_long);

  static ExpenseCategory byId(String id) =>
      all.firstWhere((c) => c.id == id, orElse: () => fallback);
}
