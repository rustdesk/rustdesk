//! Sciter.Lite interface.

#![allow(non_camel_case_types, non_snake_case)]
#![allow(dead_code)]

use capi::sctypes::*;
use capi::scdef::{GFX_LAYER, ELEMENT_BITMAP_RECEIVER};
use capi::scdom::HELEMENT;
use capi::scbehavior::{MOUSE_BUTTONS, MOUSE_EVENTS, KEY_EVENTS};


#[repr(C)]
#[derive(Copy, Clone)]
#[derive(Debug, PartialOrd, PartialEq)]
pub enum SCITER_X_MSG_CODE
{
  SXM_CREATE  = 0,
  SXM_DESTROY = 1,
  SXM_SIZE    = 2,
  SXM_PAINT   = 3,
  SXM_RESOLUTION = 4,
  SXM_HEARTBIT = 5,
  SXM_MOUSE = 6,
  SXM_KEY = 7,
  SXM_FOCUS = 8,
}

#[repr(C)]
#[derive(Debug)]
/// Common header of message structures passed to `SciterProcX`.
pub struct SCITER_X_MSG
{
	pub msg: SCITER_X_MSG_CODE,
}

impl From<SCITER_X_MSG_CODE> for SCITER_X_MSG {
	fn from(code: SCITER_X_MSG_CODE) -> Self {
		Self { msg: code }
	}
}

#[repr(C)]
#[derive(Debug)]
/// Message to create the specific Sciter backend.
pub struct SCITER_X_MSG_CREATE
{
	pub header: SCITER_X_MSG,
	pub backend: GFX_LAYER,
	pub transparent: BOOL,
}

#[repr(C)]
#[derive(Debug)]
/// Message to destroy the current Sciter backend.
pub struct SCITER_X_MSG_DESTROY
{
	pub header: SCITER_X_MSG,
}

#[repr(C)]
#[derive(Debug)]
/// Message to notify Sciter about view resize.
pub struct SCITER_X_MSG_SIZE
{
	pub header: SCITER_X_MSG,
	pub width: UINT,
	pub height: UINT,
}

#[repr(C)]
#[derive(Debug)]
/// Message to notify Sciter about screen resolution change.
pub struct SCITER_X_MSG_RESOLUTION
{
	pub header: SCITER_X_MSG,

	/// Pixels per inch.
	pub ppi: UINT,
}

#[repr(C)]
#[derive(Debug)]
/// Message to notify Sciter about mouse input.
pub struct SCITER_X_MSG_MOUSE
{
	pub header: SCITER_X_MSG,

	pub event: MOUSE_EVENTS,
	pub button: MOUSE_BUTTONS,
	pub modifiers: UINT,
	pub pos: POINT,
}

#[repr(C)]
#[derive(Debug)]
/// Message to notify Sciter about keyboard input.
pub struct SCITER_X_MSG_KEY
{
	pub header: SCITER_X_MSG,

	pub event: KEY_EVENTS,
	pub code: UINT,
	pub modifiers: UINT,
}

#[repr(C)]
#[derive(Debug)]
/// Message to notify Sciter about window focus change.
pub struct SCITER_X_MSG_FOCUS
{
	pub header: SCITER_X_MSG,

	pub enter: BOOL,
}

#[repr(C)]
#[derive(Debug)]
/// Give Sciter a chance to process animations, timers and other timed things.
pub struct SCITER_X_MSG_HEARTBIT
{
	pub header: SCITER_X_MSG,

	/// Absolute time in milliseconds.
	pub time: UINT,
}

#[repr(C)]
#[derive(Copy, Clone)]
#[derive(Debug, PartialOrd, PartialEq)]
/// `SCITER_X_MSG_PAINT` rendering targets.
pub enum SCITER_PAINT_TARGET_TYPE
{
	/// Default target - OpenGL window surface.
	SPT_DEFAULT   = 0,

	/// Bitmap target.
	SPT_RECEIVER  = 1,

	/// `IDXGISurface` target (since 4.4.3.27).
	SPT_SURFACE = 2,
}

/// Message to paint view to the provided target (HDC or callback).
#[repr(C)]
pub struct SCITER_X_MSG_PAINT
{
	pub header: SCITER_X_MSG,
	pub element: HELEMENT,
	pub isFore: BOOL,
	pub targetType: SCITER_PAINT_TARGET_TYPE,

	// union {
	// HDC or LPVOID
	pub context: LPVOID,
	pub callback: Option<ELEMENT_BITMAP_RECEIVER>,
	// }
}

/// Sciter.Lite key codes.
///
/// The table matches the [GLFW keyboard keys](https://www.glfw.org/docs/3.3/group__keys.html).
pub mod key_codes
{
	#[repr(C)]
	#[derive(Copy, Clone)]
	#[derive(Debug, PartialOrd, PartialEq)]
	/// The same keys, but wrapped as a enum.
	pub enum KEYBOARD_CODES {
  KB_SPACE              = 32,
  KB_APOSTROPHE         = 39,  /* ' */
	KB_COMMA              = 44,  /* , */
	// KB_QUOTE = KB_COMMA,
  KB_MINUS              = 45,  /* - */
  KB_PERIOD             = 46,  /* . */
  KB_SLASH              = 47,  /* / */
  KB_0                  = 48,
  KB_1                  = 49,
  KB_2                  = 50,
  KB_3                  = 51,
  KB_4                  = 52,
  KB_5                  = 53,
  KB_6                  = 54,
  KB_7                  = 55,
  KB_8                  = 56,
  KB_9                  = 57,
  KB_SEMICOLON          = 59,  /* ; */
  KB_EQUAL              = 61,  /* = */
  KB_A                  = 65,
  KB_B                  = 66,
  KB_C                  = 67,
  KB_D                  = 68,
  KB_E                  = 69,
  KB_F                  = 70,
  KB_G                  = 71,
  KB_H                  = 72,
  KB_I                  = 73,
  KB_J                  = 74,
  KB_K                  = 75,
  KB_L                  = 76,
  KB_M                  = 77,
  KB_N                  = 78,
  KB_O                  = 79,
  KB_P                  = 80,
  KB_Q                  = 81,
  KB_R                  = 82,
  KB_S                  = 83,
  KB_T                  = 84,
  KB_U                  = 85,
  KB_V                  = 86,
  KB_W                  = 87,
  KB_X                  = 88,
  KB_Y                  = 89,
  KB_Z                  = 90,
	KB_LEFT_BRACKET       = 91,  /* [ */
	// KB_LEFTBRACKET = KB_LEFT_BRACKET,
  KB_BACKSLASH          = 92,  /* \ */
	KB_RIGHT_BRACKET      = 93,  /* ] */
	// KB_RIGHTBRACKET = KB_RIGHT_BRACKET,
  KB_GRAVE_ACCENT       = 96,  /* ` */
	KB_WORLD_1            = 161, /* non-US #1 */
	// KB_DOT = KB_WORLD_1,
  KB_WORLD_2            = 162, /* non-US #2 */

/* Function keys */
  KB_ESCAPE             = 256,
	KB_ENTER              = 257,
	// KB_RETURN = KB_ENTER,
  KB_TAB                = 258,
	KB_BACKSPACE          = 259,
	// KB_BACK = KB_BACKSPACE,
  KB_INSERT             = 260,
  KB_DELETE             = 261,
  KB_RIGHT              = 262,
  KB_LEFT               = 263,
  KB_DOWN               = 264,
  KB_UP                 = 265,
  KB_PAGE_UP            = 266,
	// KB_PRIOR = KB_PAGE_UP,
  KB_PAGE_DOWN          = 267,
	// KB_NEXT = KB_PAGE_DOWN,
  KB_HOME               = 268,
  KB_END                = 269,
  KB_CAPS_LOCK          = 280,
	// KB_CAPITAL = KB_CAPS_LOCK,
  KB_SCROLL_LOCK        = 281,
	// KB_SCROLL = KB_SCROLL_LOCK,
  KB_NUM_LOCK           = 282,
	// KB_NUMLOCK = KB_NUM_LOCK,
  KB_PRINT_SCREEN       = 283,
  KB_PAUSE              = 284,
  KB_F1                 = 290,
  KB_F2                 = 291,
  KB_F3                 = 292,
  KB_F4                 = 293,
  KB_F5                 = 294,
  KB_F6                 = 295,
  KB_F7                 = 296,
  KB_F8                 = 297,
  KB_F9                 = 298,
  KB_F10                = 299,
  KB_F11                = 300,
  KB_F12                = 301,
  KB_F13                = 302,
  KB_F14                = 303,
  KB_F15                = 304,
  KB_F16                = 305,
  KB_F17                = 306,
  KB_F18                = 307,
  KB_F19                = 308,
  KB_F20                = 309,
  KB_F21                = 310,
  KB_F22                = 311,
  KB_F23                = 312,
  KB_F24                = 313,
  KB_F25                = 314,
  KB_NUMPAD0            = 320,
  KB_NUMPAD1            = 321,
  KB_NUMPAD2            = 322,
  KB_NUMPAD3            = 323,
  KB_NUMPAD4            = 324,
  KB_NUMPAD5            = 325,
  KB_NUMPAD6            = 326,
  KB_NUMPAD7            = 327,
  KB_NUMPAD8            = 328,
  KB_NUMPAD9            = 329,
  KB_NUMPAD_DECIMAL     = 330,
	// KB_DECIMAL = KB_NUMPAD_DECIMAL, KB_SEPARATOR = KB_DECIMAL,
  KB_NUMPAD_DIVIDE      = 331,
	// KB_DIVIDE = KB_NUMPAD_DIVIDE,
  KB_NUMPAD_MULTIPLY    = 332,
	// KB_MULTIPLY = KB_NUMPAD_MULTIPLY,
  KB_NUMPAD_SUBTRACT    = 333,
	// KB_SUBTRACT = KB_NUMPAD_SUBTRACT,
  KB_NUMPAD_ADD         = 334,
	// KB_ADD = KB_NUMPAD_ADD, KB_PLUS = KB_ADD,
  KB_NUMPAD_ENTER       = 335,
  KB_NUMPAD_EQUAL       = 336,
  KB_LEFT_SHIFT         = 340,
	// KB_SHIFT = KB_LEFT_SHIFT,
  KB_LEFT_CONTROL       = 341,
	// KB_CONTROL = KB_LEFT_CONTROL, KB_SHORTCUT = KB_CONTROL,
  KB_LEFT_ALT           = 342,
  KB_LEFT_SUPER         = 343,
  KB_RIGHT_SHIFT        = 344,
  KB_RIGHT_CONTROL      = 345,
  KB_RIGHT_ALT          = 346,
  KB_RIGHT_SUPER        = 347,
	KB_MENU               = 348,
}

  pub const KB_SPACE            : u32 = 32;
  pub const KB_APOSTROPHE       : u32 = 39;  /* ' */
	pub const KB_COMMA            : u32 = 44;  /* , */
	pub const KB_QUOTE : u32 = KB_COMMA;
  pub const KB_MINUS            : u32 = 45;  /* - */
  pub const KB_PERIOD           : u32 = 46;  /* . */
	pub const KB_SLASH            : u32 = 47;  /* / */

  pub const KB_0                : u32 = 48;
  pub const KB_1                : u32 = 49;
  pub const KB_2                : u32 = 50;
  pub const KB_3                : u32 = 51;
  pub const KB_4                : u32 = 52;
  pub const KB_5                : u32 = 53;
  pub const KB_6                : u32 = 54;
  pub const KB_7                : u32 = 55;
  pub const KB_8                : u32 = 56;
	pub const KB_9                : u32 = 57;

  pub const KB_SEMICOLON        : u32 = 59;  /* ; */
	pub const KB_EQUAL            : u32 = 61;  /* = */

  pub const KB_A                : u32 = 65;
  pub const KB_B                : u32 = 66;
  pub const KB_C                : u32 = 67;
  pub const KB_D                : u32 = 68;
  pub const KB_E                : u32 = 69;
  pub const KB_F                : u32 = 70;
  pub const KB_G                : u32 = 71;
  pub const KB_H                : u32 = 72;
  pub const KB_I                : u32 = 73;
  pub const KB_J                : u32 = 74;
  pub const KB_K                : u32 = 75;
  pub const KB_L                : u32 = 76;
  pub const KB_M                : u32 = 77;
  pub const KB_N                : u32 = 78;
  pub const KB_O                : u32 = 79;
  pub const KB_P                : u32 = 80;
  pub const KB_Q                : u32 = 81;
  pub const KB_R                : u32 = 82;
  pub const KB_S                : u32 = 83;
  pub const KB_T                : u32 = 84;
  pub const KB_U                : u32 = 85;
  pub const KB_V                : u32 = 86;
  pub const KB_W                : u32 = 87;
  pub const KB_X                : u32 = 88;
  pub const KB_Y                : u32 = 89;
	pub const KB_Z                : u32 = 90;

	pub const KB_LEFT_BRACKET     : u32 = 91;  /* [ */
	pub const KB_LEFTBRACKET : u32 = KB_LEFT_BRACKET;
  pub const KB_BACKSLASH        : u32 = 92;  /* \ */
	pub const KB_RIGHT_BRACKET    : u32 = 93;  /* ] */
	pub const KB_RIGHTBRACKET : u32 = KB_RIGHT_BRACKET;
  pub const KB_GRAVE_ACCENT     : u32 = 96;  /* ` */
	pub const KB_WORLD_1          : u32 = 161; /* non-US #1 */
	pub const KB_DOT : u32 = KB_WORLD_1;
  pub const KB_WORLD_2          : u32 = 162; /* non-US #2 */

/* Function keys */
  pub const KB_ESCAPE           : u32 = 256;
	pub const KB_ENTER            : u32 = 257;
	pub const KB_RETURN : u32 = KB_ENTER;
  pub const KB_TAB              : u32 = 258;
	pub const KB_BACKSPACE        : u32 = 259;
	pub const KB_BACK : u32 = KB_BACKSPACE;
  pub const KB_INSERT           : u32 = 260;
  pub const KB_DELETE           : u32 = 261;
  pub const KB_RIGHT            : u32 = 262;
  pub const KB_LEFT             : u32 = 263;
  pub const KB_DOWN             : u32 = 264;
  pub const KB_UP               : u32 = 265;
	pub const KB_PAGE_UP          : u32 = 266;
	pub const KB_PRIOR : u32 = KB_PAGE_UP;
	pub const KB_PAGE_DOWN        : u32 = 267;
	pub const KB_NEXT : u32 = KB_PAGE_DOWN;
  pub const KB_HOME             : u32 = 268;
  pub const KB_END              : u32 = 269;
	pub const KB_CAPS_LOCK        : u32 = 280;
	pub const KB_CAPITAL : u32 = KB_CAPS_LOCK;
	pub const KB_SCROLL_LOCK      : u32 = 281;
	pub const KB_SCROLL : u32 = KB_SCROLL_LOCK;
	pub const KB_NUM_LOCK         : u32 = 282;
	pub const KB_NUMLOCK : u32 = KB_NUM_LOCK;
  pub const KB_PRINT_SCREEN     : u32 = 283;
	pub const KB_PAUSE            : u32 = 284;

  pub const KB_F1               : u32 = 290;
  pub const KB_F2               : u32 = 291;
  pub const KB_F3               : u32 = 292;
  pub const KB_F4               : u32 = 293;
  pub const KB_F5               : u32 = 294;
  pub const KB_F6               : u32 = 295;
  pub const KB_F7               : u32 = 296;
  pub const KB_F8               : u32 = 297;
  pub const KB_F9               : u32 = 298;
  pub const KB_F10              : u32 = 299;
  pub const KB_F11              : u32 = 300;
  pub const KB_F12              : u32 = 301;
  pub const KB_F13              : u32 = 302;
  pub const KB_F14              : u32 = 303;
  pub const KB_F15              : u32 = 304;
  pub const KB_F16              : u32 = 305;
  pub const KB_F17              : u32 = 306;
  pub const KB_F18              : u32 = 307;
  pub const KB_F19              : u32 = 308;
  pub const KB_F20              : u32 = 309;
  pub const KB_F21              : u32 = 310;
  pub const KB_F22              : u32 = 311;
  pub const KB_F23              : u32 = 312;
  pub const KB_F24              : u32 = 313;
	pub const KB_F25              : u32 = 314;

  pub const KB_NUMPAD0          : u32 = 320;
  pub const KB_NUMPAD1          : u32 = 321;
  pub const KB_NUMPAD2          : u32 = 322;
  pub const KB_NUMPAD3          : u32 = 323;
  pub const KB_NUMPAD4          : u32 = 324;
  pub const KB_NUMPAD5          : u32 = 325;
  pub const KB_NUMPAD6          : u32 = 326;
  pub const KB_NUMPAD7          : u32 = 327;
  pub const KB_NUMPAD8          : u32 = 328;
	pub const KB_NUMPAD9          : u32 = 329;

	pub const KB_NUMPAD_DECIMAL   : u32 = 330;
	pub const KB_DECIMAL : u32 = KB_NUMPAD_DECIMAL;
	pub const KB_SEPARATOR : u32 = KB_DECIMAL;
	pub const KB_NUMPAD_DIVIDE    : u32 = 331;
	pub const KB_DIVIDE : u32 = KB_NUMPAD_DIVIDE;
	pub const KB_NUMPAD_MULTIPLY  : u32 = 332;
	pub const KB_MULTIPLY : u32 = KB_NUMPAD_MULTIPLY;
	pub const KB_NUMPAD_SUBTRACT  : u32 = 333;
	pub const KB_SUBTRACT : u32 = KB_NUMPAD_SUBTRACT;
	pub const KB_NUMPAD_ADD       : u32 = 334;
	pub const KB_ADD : u32 = KB_NUMPAD_ADD;
	pub const KB_PLUS : u32 = KB_ADD;
  pub const KB_NUMPAD_ENTER     : u32 = 335;
	pub const KB_NUMPAD_EQUAL     : u32 = 336;

	pub const KB_LEFT_SHIFT       : u32 = 340;
	pub const KB_SHIFT : u32 = KB_LEFT_SHIFT;
	pub const KB_LEFT_CONTROL     : u32 = 341;
	pub const KB_CONTROL : u32 = KB_LEFT_CONTROL;
	pub const KB_SHORTCUT : u32 = KB_CONTROL;
  pub const KB_LEFT_ALT         : u32 = 342;
  pub const KB_LEFT_SUPER       : u32 = 343;
  pub const KB_RIGHT_SHIFT      : u32 = 344;
  pub const KB_RIGHT_CONTROL    : u32 = 345;
  pub const KB_RIGHT_ALT        : u32 = 346;
  pub const KB_RIGHT_SUPER      : u32 = 347;
  pub const KB_MENU             : u32 = 348;
}
