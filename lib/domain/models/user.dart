import 'package:flutter/foundation.dart';

/// A person who can participate in groups and expenses.
///
/// In a fully online product this would map to an authenticated account; here
/// it is a local profile so the app is usable offline and across every
/// platform without a backend.
@immutable
class AppUser {
  const AppUser({
    required this.id,
    required this.name,
    this.email,
    this.avatarColor,
  });

  final String id;
  final String name;
  final String? email;

  /// ARGB colour used for the avatar chip, stored as an int for portability.
  final int? avatarColor;

  /// First grapheme of the name, used as the avatar fallback.
  String get initial => name.trim().isEmpty ? '?' : name.trim()[0].toUpperCase();

  AppUser copyWith({String? name, String? email, int? avatarColor}) => AppUser(
        id: id,
        name: name ?? this.name,
        email: email ?? this.email,
        avatarColor: avatarColor ?? this.avatarColor,
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'name': name,
        'email': email,
        'avatarColor': avatarColor,
      };

  factory AppUser.fromJson(Map<String, dynamic> json) => AppUser(
        id: json['id'] as String,
        name: json['name'] as String,
        email: json['email'] as String?,
        avatarColor: json['avatarColor'] as int?,
      );

  @override
  bool operator ==(Object other) =>
      other is AppUser &&
      other.id == id &&
      other.name == name &&
      other.email == email &&
      other.avatarColor == avatarColor;

  @override
  int get hashCode => Object.hash(id, name, email, avatarColor);
}
