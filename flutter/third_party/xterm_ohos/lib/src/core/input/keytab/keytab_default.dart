import 'package:xterm/src/core/input/keytab/keytab_parse.dart';
import 'package:xterm/src/core/input/keytab/keytab_token.dart';

const kDefaultKeytab = r'''
# [README.default.Keytab] Default Keyboard Table
#
# To customize your keyboard, copy this file to something ending with
# .keytab and change it to meet you needs.
#
# Please read the README-KeyTab and the doc/user/README.keyboard files
# in this case.
#
# --------------------------------------------------------------

keyboard "Default (XFree 4)"

# --------------------------------------------------------------
#
# Note that this particular table is a "risc" version made to
# ease customization without bothering with obsolete details.
# See VT100.keytab for the more hairy stuff.
#
# --------------------------------------------------------------

# common keys

key Escape             : "\E"

key Tab   -Shift       : "\t"
key Tab   +Shift+Ansi  : "\E[Z"
key Tab   +Shift-Ansi  : "\t"
key Backtab     +Ansi  : "\E[Z"
key Backtab     -Ansi  : "\t"

key Return-Shift-NewLine : "\r"
key Return-Shift+NewLine : "\r\n"

key Return+Shift         : "\EOM"

key Backspace  +Alt : "\x17"

# Backspace and Delete codes are preserving CTRL-H.
#
# Backspace without CTRL sends '^?'; this matches XTerm behaviour, so that
# pressing Alt+Backspace will send \E + Del, which is the expected behaviour
# in some apps (e.g. emacs), and it was the behaviour before the commit
# that add the Backspace +Control rule
key Backspace   -Control : "\x7f"

# Match xterm behaviour: Backspace sends '^H' when Control is pressed
# BS, hex \x08, \b
key Backspace  +Control : "\b"

# Arrow keys in VT52 mode
# shift up/down are reserved for scrolling.
# shift left/right are reserved for switching between tabs (this is hardcoded).

key Up   -Shift-Ansi : "\EA"
key Down -Shift-Ansi : "\EB"
key Right-Shift-Ansi : "\EC"
key Left -Shift-Ansi : "\ED"

# Arrow keys in ANSI mode with Application - and Normal Cursor Mode)

key Up    -Shift-AnyMod+Ansi+AppCuKeys : "\EOA"
key Down  -Shift-AnyMod+Ansi+AppCuKeys : "\EOB"
key Right -Shift-AnyMod+Ansi+AppCuKeys : "\EOC"
key Left  -Shift-AnyMod+Ansi+AppCuKeys : "\EOD"

key Up    -Shift-AnyMod+Ansi-AppCuKeys : "\E[A"
key Down  -Shift-AnyMod+Ansi-AppCuKeys : "\E[B"
key Right -Shift-AnyMod+Ansi-AppCuKeys : "\E[C"
key Left  -Shift-AnyMod+Ansi-AppCuKeys : "\E[D"

key Up    -Shift+AnyMod+Ansi           : "\E[1;5A"
key Down  -Shift+AnyMod+Ansi           : "\E[1;5B"

# Right / Left with Control
key Right -Shift-Alt+Control+Ansi      : "\E[1;5C"
key Left  -Shift-Alt+Control+Ansi      : "\E[1;5D"

# Right / Left with Alt not on a Mac
key Right -Shift+Alt-Control+Ansi-Mac  : "\E[1;5C"
key Left  -Shift+Alt-Control+Ansi-Mac  : "\E[1;5D"

# Right / Left with Alt on a Mac
key Right -Shift+Alt-Control+Ansi+Mac  : "\Ef"
key Left  -Shift+Alt-Control+Ansi+Mac  : "\Eb"

key Up    +Shift+AppScreen             : "\E[1;*A"
key Down  +Shift+AppScreen             : "\E[1;*B"
key Left  +Shift+AppScreen             : "\E[1;*D"
key Right +Shift+AppScreen             : "\E[1;*C"

# Keypad keys with NumLock ON
# (see https://web.archive.org/web/20070807181942/http://www.nw.com/nw/WWW/products/wizcon/vt100.html
#    https://vt100.net/docs/vt100-ug/chapter3.html)
#
# Not enabled for now because it breaks the keypad in Vim.
#
#key 0 +KeyPad+AppKeyPad : "\EOp"
#key 1 +KeyPad+AppKeyPad : "\EOq"
#key 2 +KeyPad+AppKeyPad : "\EOr"
#key 3 +KeyPad+AppKeyPad : "\EOs"
#key 4 +KeyPad+AppKeyPad : "\EOt"
#key 5 +KeyPad+AppKeyPad : "\EOu"
#key 6 +KeyPad+AppKeyPad : "\EOv"
#key 7 +KeyPad+AppKeyPad : "\EOw"
#key 8 +KeyPad+AppKeyPad : "\EOx"
#key 9 +KeyPad+AppKeyPad : "\EOy"
#key + +KeyPad+AppKeyPad : "\EOl"
#key - +KeyPad+AppKeyPad : "\EOm"
#key . +KeyPad+AppKeyPad : "\EOn"
#key * +KeyPad+AppKeyPad : "\EOM"
#key Enter +KeyPad+AppKeyPad : "\r"

# Keypad keys with NumLock Off
key Up    -Shift+Ansi+AppCuKeys+KeyPad : "\EOA"
key Down  -Shift+Ansi+AppCuKeys+KeyPad : "\EOB"
key Right -Shift+Ansi+AppCuKeys+KeyPad : "\EOC"
key Left  -Shift+Ansi+AppCuKeys+KeyPad : "\EOD"

key Up    -Shift+Ansi-AppCuKeys+KeyPad : "\E[A"
key Down  -Shift+Ansi-AppCuKeys+KeyPad : "\E[B"
key Right -Shift+Ansi-AppCuKeys+KeyPad : "\E[C"
key Left  -Shift+Ansi-AppCuKeys+KeyPad : "\E[D"

key Home        +AppCuKeys+KeyPad : "\EOH"
key End         +AppCuKeys+KeyPad : "\EOF"
key Home        -AppCuKeys+KeyPad : "\E[H"
key End         -AppCuKeys+KeyPad : "\E[F"

key Insert      +KeyPad  : "\E[2~"
key Delete      +KeyPad  : "\E[3~"
key PgUp -Shift+KeyPad  : "\E[5~"
key PgDown  -Shift+KeyPad  : "\E[6~"

# the key labelled 5 on the Keypad, is Qt::Key_Clear (a very intuitive
# and discoverable name...)
key Clear       +KeyPad : "\E[E"

# other grey PC keys

key Enter+NewLine : "\r\n"
key Enter-NewLine : "\r"

key NumEnter+NewLine : "\r\n"
key NumEnter-NewLine : "\r"

key Home        -AnyMod-AppCuKeys : "\E[H"
key End         -AnyMod-AppCuKeys : "\E[F"
key Home        -AnyMod+AppCuKeys : "\EOH"
key End         -AnyMod+AppCuKeys : "\EOF"
key Home        +AnyMod           : "\E[1;*H"
key End         +AnyMod           : "\E[1;*F"

key Insert      -AnyMod  : "\E[2~"
key Delete      -AnyMod  : "\E[3~"
key Insert      +AnyMod  : "\E[2;*~"
key Delete      +AnyMod  : "\E[3;*~"

key PgUp -Shift-AnyMod  : "\E[5~"
key PgDown  -Shift-AnyMod  : "\E[6~"
key PgUp -Shift+AnyMod  : "\E[5;*~"
key PgDown  -Shift+AnyMod  : "\E[6;*~"

# Function keys
key F1  -AnyMod  : "\EOP"
key F2  -AnyMod  : "\EOQ"
key F3  -AnyMod  : "\EOR"
key F4  -AnyMod  : "\EOS"
key F5  -AnyMod  : "\E[15~"
key F6  -AnyMod  : "\E[17~"
key F7  -AnyMod  : "\E[18~"
key F8  -AnyMod  : "\E[19~"
key F9  -AnyMod  : "\E[20~"
key F10 -AnyMod  : "\E[21~"
key F11 -AnyMod  : "\E[23~"
key F12 -AnyMod  : "\E[24~"

key F1  +AnyMod  : "\EO*P"
key F2  +AnyMod  : "\EO*Q"
key F3  +AnyMod  : "\EO*R"
key F4  +AnyMod  : "\EO*S"
key F5  +AnyMod  : "\E[15;*~"
key F6  +AnyMod  : "\E[17;*~"
key F7  +AnyMod  : "\E[18;*~"
key F8  +AnyMod  : "\E[19;*~"
key F9  +AnyMod  : "\E[20;*~"
key F10 +AnyMod  : "\E[21;*~"
key F11 +AnyMod  : "\E[23;*~"
key F12 +AnyMod  : "\E[24;*~"

# Work around dead keys

key Space +Control : "\x00"

# Some keys are used by konsole to cause operations.
# The scroll* operations refer to the history buffer.

key Up    +Shift-AppScreen  : scrollLineUp
key PgUp  +Shift-AppScreen  : scrollPageUp
key Home  +Shift-AppScreen  : scrollUpToTop
key Down  +Shift-AppScreen  : scrollLineDown
key PgDown  +Shift-AppScreen  : scrollPageDown
key End   +Shift-AppScreen  : scrollDownToBottom
''';

void main() {
  final tokens = tokenize(kDefaultKeytab).toList();
  final parser = KeytabParser()..addTokens(tokens);
  print(parser.result);
}
