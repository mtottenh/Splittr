import 'package:flutter/material.dart';

import '../../domain/models/user.dart';

/// Circular avatar showing the user's initial over their assigned colour.
class UserAvatar extends StatelessWidget {
  const UserAvatar({super.key, required this.user, this.radius = 20});

  final AppUser user;
  final double radius;

  @override
  Widget build(BuildContext context) {
    final color = user.avatarColor != null
        ? Color(user.avatarColor!)
        : Theme.of(context).colorScheme.primary;
    return CircleAvatar(
      radius: radius,
      backgroundColor: color,
      child: Text(
        user.initial,
        style: TextStyle(
          color: Colors.white,
          fontWeight: FontWeight.bold,
          fontSize: radius * 0.8,
        ),
      ),
    );
  }
}
