import 'package:flutter/material.dart';

/// Circular avatar showing a person's initial over a colour derived from their
/// id, so the same person always gets the same colour without the engine having
/// to store one.
class UserAvatar extends StatelessWidget {
  const UserAvatar({
    super.key,
    required this.name,
    this.id,
    this.radius = 20,
  });

  final String name;
  final String? id;
  final double radius;

  static const _palette = <int>[
    0xFF1CC29F, // teal (brand)
    0xFFEF6C57,
    0xFF5B8DEF,
    0xFFF2A93B,
    0xFF9B6CD6,
    0xFF3FB68B,
    0xFFE05A8E,
    0xFF4C9AA8,
  ];

  Color get _color {
    final key = id ?? name;
    if (key.isEmpty) return Color(_palette.first);
    final hash = key.codeUnits.fold<int>(0, (h, c) => (h * 31 + c) & 0x7fffffff);
    return Color(_palette[hash % _palette.length]);
  }

  String get _initial =>
      name.trim().isEmpty ? '?' : name.trim()[0].toUpperCase();

  @override
  Widget build(BuildContext context) {
    return CircleAvatar(
      radius: radius,
      backgroundColor: _color,
      child: Text(
        _initial,
        style: TextStyle(
          color: Colors.white,
          fontWeight: FontWeight.bold,
          fontSize: radius * 0.8,
        ),
      ),
    );
  }
}
