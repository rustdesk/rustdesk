// ignore_for_file: constant_identifier_names

abstract class Ascii {
  /*
   * Helper functions
   */

  static bool isNonPrintable(int c) {
    return c < 32 || c == 127;
  }

  /*
   * Non-printable ASCII characters
   */

  ///  Null character
  static const NULL = 00;

  ///  Start of Header
  static const SOH = 01;

  ///  Start of Text
  static const STX = 02;

  ///  End of Text, hearts card suit
  static const ETX = 03;

  ///  End of Transmission, diamonds card suit
  static const EOT = 04;

  ///  Enquiry, clubs card suit
  static const ENQ = 05;

  ///  Acknowledgement, spade card suit
  static const ACK = 06;

  ///  Bell
  static const BEL = 07;

  ///  Backspace
  static const BS = 08;

  ///  Horizontal Tab
  static const HT = 09;

  ///  Line feed
  static const LF = 10;

  ///  Vertical Tab, male symbol, symbol for Mars
  static const VT = 11;

  ///  Form feed, female symbol, symbol for Venus
  static const FF = 12;

  ///  Carriage return
  static const CR = 13;

  ///  Shift Out
  static const SO = 14;

  ///  Shift In
  static const SI = 15;

  ///  Data link escape
  static const DLE = 16;

  ///  Device control 1
  static const DC1 = 17;

  ///  Device control 2
  static const DC2 = 18;

  ///  Device control 3
  static const DC3 = 19;

  ///  Device control 4
  static const DC4 = 20;

  ///  NAK Negative-acknowledge
  static const NAK = 21;

  ///  Synchronous idle
  static const SYN = 22;

  ///  End of trans. block
  static const ETB = 23;

  ///  Cancel
  static const CAN = 24;

  ///  End of medium
  static const EM = 25;

  ///  Substitute
  static const SUB = 26;

  ///  Escape
  static const ESC = 27;

  ///  File separator
  static const FS = 28;

  ///  Group separator
  static const GS = 29;

  ///  Record separator
  static const RS = 30;

  ///  Unit separator
  static const US = 31;

  ///  Delete
  static const DEL = 127;

  /*
   * Printable ASCII characters
   */

  /// Space " "
  static const space = 32;

  /// Exclamation mark "!"
  static const exclamationMark = 33;

  /// Double quotes '"'
  static const doubleQuotes = 34;

  /// Number sign '#'
  static const numberSign = 35;

  /// Dollar sign '$'
  static const dollarSign = 36;

  /// Percent sign '%'
  static const percentSign = 37;

  /// Ampersand '&'
  static const ampersand = 38;

  /// Single quote "'"
  static const singleQuote = 39;

  /// round brackets or parentheses, opening round bracket '('
  static const openParentheses = 40;

  /// parentheses or round brackets, closing parentheses ')'
  static const closeParentheses = 41;

  /// Asterisk '*'
  static const asterisk = 42;

  /// Plus sign '+'
  static const plus = 43;

  /// Comma ","
  static const comma = 44;

  /// Hyphen , minus sign '-'
  static const minus = 45;

  /// Dot, full stop '.'
  static const dot = 46;

  /// Slash , forward slash , fraction bar , division slash '/'
  static const slash = 47;

  /// number zero
  static const num0 = 48;

  /// number one
  static const num1 = 49;

  /// number two
  static const num2 = 50;

  /// number three
  static const num3 = 51;

  /// number four
  static const num4 = 52;

  /// number five
  static const num5 = 53;

  /// number six
  static const num6 = 54;

  /// number seven
  static const num7 = 55;

  /// number eight
  static const num8 = 56;

  /// number nine
  static const num9 = 57;

  /// Colon ':'
  static const colon = 58;

  /// Semicolon ';'
  static const semicolon = 59;

  /// Less-than sign '<'
  static const lessThan = 60;

  /// Equals sign '='
  static const equal = 61;

  /// Greater-than sign ; Inequality sign '>'
  static const greaterThan = 62;

  /// Question mark '?'
  static const questionMark = 63;

  /// At sign '@'
  static const atSign = 64;

  /// Capital letter A
  static const A = 65;

  /// Capital letter B
  static const B = 66;

  /// Capital letter C
  static const C = 67;

  /// Capital letter D
  static const D = 68;

  /// Capital letter E
  static const E = 69;

  /// Capital letter F
  static const F = 70;

  /// Capital letter G
  static const G = 71;

  /// Capital letter H
  static const H = 72;

  /// Capital letter I
  static const I = 73;

  /// Capital letter J
  static const J = 74;

  /// Capital letter K
  static const K = 75;

  /// Capital letter L
  static const L = 76;

  /// Capital letter M
  static const M = 77;

  /// Capital letter N
  static const N = 78;

  /// Capital letter O
  static const O = 79;

  /// Capital letter P
  static const P = 80;

  /// Capital letter Q
  static const Q = 81;

  /// Capital letter R
  static const R = 82;

  /// Capital letter S
  static const S = 83;

  /// Capital letter T
  static const T = 84;

  /// Capital letter U
  static const U = 85;

  /// Capital letter V
  static const V = 86;

  /// Capital letter W
  static const W = 87;

  /// Capital letter X
  static const X = 88;

  /// Capital letter Y
  static const Y = 89;

  /// Capital letter Z
  static const Z = 90;

  /// square brackets or box brackets, opening bracket '['
  static const openBracket = 91;

  /// Backslash , reverse slash '\\'
  static const backslash = 92;

  /// box brackets or square brackets, closing bracket ']'
  static const closeBracket = 93;

  /// Circumflex accent or Caret  '^'
  static const caret = 94;

  /// underscore , understrike , underbar or low line '_'
  static const underscore = 95;

  /// Grave accent  '`'
  static const graveAccent = 96;

  /// Lowercase letter a , minuscule a
  static const a = 97;

  /// Lowercase letter b , minuscule b
  static const b = 98;

  /// Lowercase letter c , minuscule c
  static const c = 99;

  /// Lowercase letter d , minuscule d
  static const d = 100;

  /// Lowercase letter e , minuscule e
  static const e = 101;

  /// Lowercase letter f , minuscule f
  static const f = 102;

  /// Lowercase letter g , minuscule g
  static const g = 103;

  /// Lowercase letter h , minuscule h
  static const h = 104;

  /// Lowercase letter i , minuscule i
  static const i = 105;

  /// Lowercase letter j , minuscule j
  static const j = 106;

  /// Lowercase letter k , minuscule k
  static const k = 107;

  /// Lowercase letter l , minuscule l
  static const l = 108;

  /// Lowercase letter m , minuscule m
  static const m = 109;

  /// Lowercase letter n , minuscule n
  static const n = 110;

  /// Lowercase letter o , minuscule o
  static const o = 111;

  /// Lowercase letter p , minuscule p
  static const p = 112;

  /// Lowercase letter q , minuscule q
  static const q = 113;

  /// Lowercase letter r , minuscule r
  static const r = 114;

  /// Lowercase letter s , minuscule s
  static const s = 115;

  /// Lowercase letter t , minuscule t
  static const t = 116;

  /// Lowercase letter u , minuscule u
  static const u = 117;

  /// Lowercase letter v , minuscule v
  static const v = 118;

  /// Lowercase letter w , minuscule w
  static const w = 119;

  /// Lowercase letter x , minuscule x
  static const x = 120;

  /// Lowercase letter y , minuscule y
  static const y = 121;

  /// Lowercase letter z , minuscule z
  static const z = 122;

  /// braces or curly brackets, opening braces '{'
  static const openBrace = 123;

  /// vertical-bar, vbar, vertical line or vertical slash '|'
  static const verticalBar = 124;

  /// curly brackets or braces, closing curly brackets '}'
  static const closeBrace = 125;

  /// Tilde ; swung dash '~'
  static const tilde = 126;
}
