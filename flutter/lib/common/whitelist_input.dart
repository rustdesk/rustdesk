final _separators = RegExp(r"[\s,;\n]+");

/// The entries typed into the IP or ID whitelist dialog, which accepts commas,
/// semicolons and whitespace between them.
///
/// Empty input clears the whitelist and gives an empty list. Input that holds
/// only separators gives `null`: it names no entry, and saving it as a cleared
/// whitelist would turn off a restriction that the server reads from the same
/// text as a list that lets no one in.
List<String>? whitelistInputEntries(String input) {
  final text = input.trim();
  if (text.isEmpty) return [];
  final entries = text.split(_separators).where((e) => e.isNotEmpty).toList();
  return entries.isEmpty ? null : entries;
}

/// The text the IP or ID whitelist dialog shows for the stored option
/// [stored], one entry per line; [cleared] is the stored value of a cleared
/// whitelist, which shows as an empty field.
String whitelistDialogText(String stored, String cleared) =>
    stored == cleared ? '' : stored.split(',').join('\n');

/// Whether the stored whitelist option [stored] is on but names no entry, so
/// that the server lets no one in. Like the server's `parse_whitelist`, a
/// value of commas only is off, and any other value is split at commas,
/// semicolons and whitespace.
bool whitelistLetsNoOneIn(String stored) =>
    stored.runes.any((c) => c != 0x2c) &&
    stored.split(_separators).every((e) => e.isEmpty);

/// Whether the IP or ID whitelist dialog, opened on the stored option [stored]
/// and showing [shown] for it, keeps [stored] when OK is pressed with [input].
///
/// A whitelist that lets no one in, such as `" , "`, shows as blank lines.
/// Submitted unchanged, those trim to an empty field, which would clear the
/// whitelist; the stored value is kept instead. Clearing the field, or the
/// Clear button, still clears it.
bool keepsStoredWhitelist(String stored, String shown, String input) =>
    input == shown && whitelistLetsNoOneIn(stored);
