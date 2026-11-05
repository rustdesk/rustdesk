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
  final entries =
      text.split(RegExp(r"[\s,;\n]+")).where((e) => e.isNotEmpty).toList();
  return entries.isEmpty ? null : entries;
}
