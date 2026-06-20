import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:path_provider/path_provider.dart';

/// Abstraction over "where the serialized app data lives".
///
/// Keeping this behind an interface means the domain and state layers never
/// touch `dart:io` or `path_provider` directly, so they stay testable and a
/// different backend (SQLite, a REST API, browser storage) can be dropped in
/// without touching anything above this line.
abstract interface class Persistence {
  Future<String?> read();
  Future<void> write(String contents);
}

/// Stores data as a single JSON document in the per-app documents directory.
/// Works on iOS, Android, Windows, Linux and macOS via path_provider.
class FilePersistence implements Persistence {
  FilePersistence({this.fileName = 'splittr_data.json'});

  final String fileName;
  File? _cachedFile;

  Future<File> _file() async {
    if (_cachedFile != null) return _cachedFile!;
    final dir = await getApplicationDocumentsDirectory();
    return _cachedFile = File('${dir.path}/$fileName');
  }

  @override
  Future<String?> read() async {
    final file = await _file();
    if (!await file.exists()) return null;
    final contents = await file.readAsString();
    return contents.isEmpty ? null : contents;
  }

  @override
  Future<void> write(String contents) async {
    final file = await _file();
    // Write to a temp file then rename for a crash-safe atomic replace.
    final tmp = File('${file.path}.tmp');
    await tmp.writeAsString(contents, flush: true);
    await tmp.rename(file.path);
  }
}

/// Volatile backend used for tests and as a graceful fallback on platforms
/// where a documents directory is unavailable (e.g. web).
class InMemoryPersistence implements Persistence {
  InMemoryPersistence([this._contents]);
  String? _contents;

  @override
  Future<String?> read() async => _contents;

  @override
  Future<void> write(String contents) async => _contents = contents;
}

/// Pretty-printed JSON encoder reused across the data layer.
const jsonEncoder = JsonEncoder.withIndent('  ');
