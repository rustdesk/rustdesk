//! Sciter's platform independent graphics interface.

#![allow(non_camel_case_types, non_snake_case)]
#![allow(dead_code)]

use capi::scdom::HELEMENT;
use capi::sctypes::{BOOL, LPCBYTE, LPCWSTR, LPVOID, UINT};
use capi::scvalue::VALUE;

MAKE_HANDLE!(#[doc = "Graphics native handle."] HGFX, _HGFX);
MAKE_HANDLE!(#[doc = "Image native handle."] HIMG, _HIMG);
MAKE_HANDLE!(#[doc = "Path native handle."] HPATH, _HPATH);
MAKE_HANDLE!(#[doc = "Text native handle."] HTEXT, _HTEXT);

pub type SC_REAL = f32;
pub type SC_POS = SC_REAL;
pub type SC_DIM = SC_REAL;
pub type SC_ANGLE = SC_REAL;

pub type SC_COLOR = u32;


#[repr(C)]
#[derive(Debug, PartialEq)]
/// Type of the result value for Sciter Graphics functions.
pub enum GRAPHIN_RESULT {
	/// E.g. not enough memory.
  PANIC = -1,
  /// Success.
  OK = 0,
  /// Bad parameter.
  BAD_PARAM = 1,
  /// Operation failed, e.g. `restore()` without `save()`.
  FAILURE = 2,
  /// Platform does not support the requested feature.
  NOTSUPPORTED = 3,
}

impl std::error::Error for GRAPHIN_RESULT {}

impl std::fmt::Display for GRAPHIN_RESULT {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{:?}", self)
	}
}


/// Path drawing mode.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum DRAW_PATH {
  /// Draw without outline line.
  FILL_ONLY = 1,
  /// Draw outline without fill.
  STROKE_ONLY = 2,
  /// Draw outlined and filled path.
  FILL_AND_STROKE = 3,
}

/// Line drawing join mode.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum LINE_JOIN {
  /// Specifies a mitered join. This produces a sharp corner or a clipped corner,
  /// depending on whether the length of the miter exceeds the miter limit (`10.0`).
  MITER = 0,
  /// Specifies a circular join. This produces a smooth, circular arc between the lines.
  ROUND = 1,
  /// Specifies a beveled join. This produces a diagonal corner.
  BEVEL = 2,
  /// Specifies a mitered join. This produces a sharp corner or a beveled corner,
  /// depending on whether the length of the miter exceeds the miter limit (`10.0`).
  MITER_OR_BEVEL = 3,
}

/// Line drawing cap mode.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum LINE_CAP {
  /// The ends of lines are squared off at the endpoints.
  BUTT = 0,
  /// The ends of lines are squared off by adding a box with an equal width
  /// and half the height of the line's thickness.
  SQUARE = 1,
  /// The ends of lines are rounded.
  ROUND = 2,
}

#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum IMAGE_ENCODING {
  RAW, // [a,b,g,r,a,b,g,r,...] vector
  PNG,
  JPG,
  WEBP,
}

#[repr(C)]
#[derive(Debug)]
pub struct SC_COLOR_STOP {
  pub color: SC_COLOR,
  pub offset: f32,
}


#[repr(C)]
#[allow(missing_docs)]
pub struct SciterGraphicsAPI {
  // image primitives
  pub imageCreate: extern "system" fn(poutImg: &mut HIMG, width: UINT, height: UINT, withAlpha: BOOL) -> GRAPHIN_RESULT,

  // construct image from B[n+0],G[n+1],R[n+2],A[n+3] data.
  // Size of pixmap data is pixmapWidth*pixmapHeight*4
  pub imageCreateFromPixmap:
    extern "system" fn(poutImg: &mut HIMG, pixmapWidth: UINT, pixmapHeight: UINT, withAlpha: BOOL, pixmap: LPCBYTE) -> GRAPHIN_RESULT,

  pub imageAddRef: extern "system" fn(himg: HIMG) -> GRAPHIN_RESULT,

  pub imageRelease: extern "system" fn(himg: HIMG) -> GRAPHIN_RESULT,

  pub imageGetInfo: extern "system" fn(himg: HIMG, width: &mut UINT, height: &mut UINT, usesAlpha: &mut BOOL) -> GRAPHIN_RESULT,

  pub imageClear: extern "system" fn(himg: HIMG, byColor: SC_COLOR) -> GRAPHIN_RESULT,

  pub imageLoad: extern "system" fn(bytes: LPCBYTE, num_bytes: UINT, pout_img: &mut HIMG) -> GRAPHIN_RESULT, // load png/jpeg/etc. image from stream of bytes

  pub imageSave: extern "system" fn(himg: HIMG, pfn: ImageWriteFunction, prm: LPVOID, encoding: IMAGE_ENCODING, quality: UINT) -> GRAPHIN_RESULT,

  // SECTION: graphics primitives and drawing operations

  // create SC_COLOR value
  pub RGBA: extern "system" fn(red: UINT, green: UINT, blue: UINT, alpha: UINT) -> SC_COLOR,

  pub gCreate: extern "system" fn(img: HIMG, pout_gfx: &mut HGFX) -> GRAPHIN_RESULT,

  pub gAddRef: extern "system" fn(gfx: HGFX) -> GRAPHIN_RESULT,

  pub gRelease: extern "system" fn(gfx: HGFX) -> GRAPHIN_RESULT,

  // Draws line from x1,y1 to x2,y2 using current lineColor and lineGradient.
  pub gLine: extern "system" fn(hgfx: HGFX, x1: SC_POS, y1: SC_POS, x2: SC_POS, y2: SC_POS) -> GRAPHIN_RESULT,

  // Draws rectangle using current lineColor/lineGradient and fillColor/fillGradient with (optional) rounded corners.
  pub gRectangle: extern "system" fn(hgfx: HGFX, x1: SC_POS, y1: SC_POS, x2: SC_POS, y2: SC_POS) -> GRAPHIN_RESULT,

  // Draws rounded rectangle using current lineColor/lineGradient and fillColor/fillGradient with (optional) rounded corners.
  pub gRoundedRectangle: extern "system" fn(
    hgfx: HGFX,
    x1: SC_POS,
    y1: SC_POS,
    x2: SC_POS,
    y2: SC_POS,
    radii8: *const SC_DIM,
  ) -> GRAPHIN_RESULT,

  // Draws circle or ellipse using current lineColor/lineGradient and fillColor/fillGradient.
  pub gEllipse: extern "system" fn(hgfx: HGFX, x: SC_POS, y: SC_POS, rx: SC_DIM, ry: SC_DIM) -> GRAPHIN_RESULT,

  // Draws closed arc using current lineColor/lineGradient and fillColor/fillGradient.
  pub gArc:
    extern "system" fn(hgfx: HGFX, x: SC_POS, y: SC_POS, rx: SC_POS, ry: SC_POS, start: SC_ANGLE, sweep: SC_ANGLE) -> GRAPHIN_RESULT,

  // Draws star.
  pub gStar: extern "system" fn(hgfx: HGFX, x: SC_POS, y: SC_POS, r1: SC_DIM, r2: SC_DIM, start: SC_ANGLE, rays: UINT) -> GRAPHIN_RESULT,

  // Closed polygon.
  pub gPolygon: extern "system" fn(hgfx: HGFX, xy: *const SC_POS, num_points: UINT) -> GRAPHIN_RESULT,

  // Polyline.
  pub gPolyline: extern "system" fn(hgfx: HGFX, xy: *const SC_POS, num_points: UINT) -> GRAPHIN_RESULT,

  // SECTION: Path operations
  pub pathCreate: extern "system" fn(path: &mut HPATH) -> GRAPHIN_RESULT,

  pub pathAddRef: extern "system" fn(path: HPATH) -> GRAPHIN_RESULT,

  pub pathRelease: extern "system" fn(path: HPATH) -> GRAPHIN_RESULT,

  pub pathMoveTo: extern "system" fn(path: HPATH, x: SC_POS, y: SC_POS, relative: BOOL) -> GRAPHIN_RESULT,

  pub pathLineTo: extern "system" fn(path: HPATH, x: SC_POS, y: SC_POS, relative: BOOL) -> GRAPHIN_RESULT,

  pub pathArcTo: extern "system" fn(
    path: HPATH,
    x: SC_POS,
    y: SC_POS,
    angle: SC_ANGLE,
    rx: SC_DIM,
    ry: SC_DIM,
    is_large_arc: BOOL,
    clockwise: BOOL,
    relative: BOOL,
  ) -> GRAPHIN_RESULT,

  pub pathQuadraticCurveTo: extern "system" fn(path: HPATH, xc: SC_POS, yc: SC_POS, x: SC_POS, y: SC_POS, relative: BOOL) -> GRAPHIN_RESULT,

  pub pathBezierCurveTo:
    extern "system" fn(path: HPATH, xc1: SC_POS, yc1: SC_POS, xc2: SC_POS, yc2: SC_POS, x: SC_POS, y: SC_POS, relative: BOOL)
      -> GRAPHIN_RESULT,

  pub pathClosePath: extern "system" fn(path: HPATH) -> GRAPHIN_RESULT,

  pub gDrawPath: extern "system" fn(hgfx: HGFX, path: HPATH, dpm: DRAW_PATH) -> GRAPHIN_RESULT,

  // end of path opearations

// SECTION: affine tranformations:
  pub gRotate: extern "system" fn(hgfx: HGFX, radians: SC_ANGLE, cx: Option<&SC_POS>, cy: Option<&SC_POS>) -> GRAPHIN_RESULT,

  pub gTranslate: extern "system" fn(hgfx: HGFX, cx: SC_POS, cy: SC_POS) -> GRAPHIN_RESULT,

  pub gScale: extern "system" fn(hgfx: HGFX, x: SC_DIM, y: SC_DIM) -> GRAPHIN_RESULT,

  pub gSkew: extern "system" fn(hgfx: HGFX, dx: SC_DIM, dy: SC_DIM) -> GRAPHIN_RESULT,

  // all above in one shot
  pub gTransform:
    extern "system" fn(hgfx: HGFX, m11: SC_POS, m12: SC_POS, m21: SC_POS, m22: SC_POS, dx: SC_POS, dy: SC_POS) -> GRAPHIN_RESULT,

  // end of affine tranformations.

// SECTION: state save/restore
  pub gStateSave: extern "system" fn(hgfx: HGFX) -> GRAPHIN_RESULT,

  pub gStateRestore: extern "system" fn(hgfx: HGFX) -> GRAPHIN_RESULT,

  // end of state save/restore

// SECTION: drawing attributes

  // set line width for subsequent drawings.
  pub gLineWidth: extern "system" fn(hgfx: HGFX, width: SC_DIM) -> GRAPHIN_RESULT,

  pub gLineJoin: extern "system" fn(hgfx: HGFX, join_type: LINE_JOIN) -> GRAPHIN_RESULT,

  pub gLineCap: extern "system" fn(hgfx: HGFX, cap_type: LINE_CAP) -> GRAPHIN_RESULT,

  // SC_COLOR for solid lines/strokes
  pub gLineColor: extern "system" fn(hgfx: HGFX, color: SC_COLOR) -> GRAPHIN_RESULT,

  // SC_COLOR for solid fills
  pub gFillColor: extern "system" fn(hgfx: HGFX, color: SC_COLOR) -> GRAPHIN_RESULT,

  // setup parameters of linear gradient of lines.
  pub gLineGradientLinear:
    extern "system" fn(hgfx: HGFX, x1: SC_POS, y1: SC_POS, x2: SC_POS, y2: SC_POS, stops: *const SC_COLOR_STOP, nstops: UINT)
      -> GRAPHIN_RESULT,

  // setup parameters of linear gradient of fills.
  pub gFillGradientLinear:
    extern "system" fn(hgfx: HGFX, x1: SC_POS, y1: SC_POS, x2: SC_POS, y2: SC_POS, stops: *const SC_COLOR_STOP, nstops: UINT)
      -> GRAPHIN_RESULT,

  // setup parameters of line gradient radial fills.
  pub gLineGradientRadial:
    extern "system" fn(hgfx: HGFX, x: SC_POS, y: SC_POS, rx: SC_DIM, ry: SC_DIM, stops: *const SC_COLOR_STOP, nstops: UINT)
      -> GRAPHIN_RESULT,

  // setup parameters of gradient radial fills.
  pub gFillGradientRadial:
    extern "system" fn(hgfx: HGFX, x: SC_POS, y: SC_POS, rx: SC_DIM, ry: SC_DIM, stops: *const SC_COLOR_STOP, nstops: UINT)
      -> GRAPHIN_RESULT,

  pub gFillMode: extern "system" fn(hgfx: HGFX, even_odd: BOOL) -> GRAPHIN_RESULT,

  // SECTION: text

  // create text layout for host element
  pub textCreateForElement: extern "system" fn(ptext: &mut HTEXT, text: LPCWSTR, textLength: UINT, he: HELEMENT, classNameOrNull: LPCWSTR) -> GRAPHIN_RESULT,

  // create text layout using explicit style declaration
  pub textCreateForElementAndStyle:
    extern "system" fn(ptext: &mut HTEXT, text: LPCWSTR, textLength: UINT, he: HELEMENT, style: LPCWSTR, styleLength: UINT) -> GRAPHIN_RESULT,

  // since 4.1.10
  pub textAddRef: extern "system" fn(text: HTEXT) -> GRAPHIN_RESULT,

  // since 4.1.10
  pub textRelease: extern "system" fn(text: HTEXT) -> GRAPHIN_RESULT,

  pub textGetMetrics: extern "system" fn(
    text: HTEXT,
    minWidth: &mut SC_DIM,
    maxWidth: &mut SC_DIM,
    height: &mut SC_DIM,
    ascent: &mut SC_DIM,
    descent: &mut SC_DIM,
    nLines: &mut UINT,
  ) -> GRAPHIN_RESULT,

  pub textSetBox: extern "system" fn(text: HTEXT, width: SC_DIM, height: SC_DIM) -> GRAPHIN_RESULT,

  // draw text with position (1..9 on MUMPAD) at px,py
  // Ex: gDrawText(100,100,5) will draw text box with its center at 100,100 px
  pub gDrawText: extern "system" fn(hgfx: HGFX, text: HTEXT, px: SC_POS, py: SC_POS, position: UINT) -> GRAPHIN_RESULT,

  // SECTION: image rendering

  // draws img onto the graphics surface with current transformation applied (scale, rotation).
  #[allow(clippy::type_complexity)]
  pub gDrawImage: extern "system" fn(
    hgfx: HGFX,
    himg: HIMG,
    x: SC_POS,
    y: SC_POS,
    w: Option<&SC_DIM>,
    h: Option<&SC_DIM>,
    ix: Option<&UINT>,
    iy: Option<&UINT>,
    iw: Option<&UINT>,
    ih: Option<&UINT>,
    opacity: Option<&f32>,
  ) -> GRAPHIN_RESULT,

  // SECTION: coordinate space
  pub gWorldToScreen: extern "system" fn(hgfx: HGFX, inout_x: &mut SC_POS, inout_y: &mut SC_POS) -> GRAPHIN_RESULT,

  pub gScreenToWorld: extern "system" fn(hgfx: HGFX, inout_x: &mut SC_POS, inout_y: &mut SC_POS) -> GRAPHIN_RESULT,

  // SECTION: clipping
  pub gPushClipBox: extern "system" fn(hgfx: HGFX, x1: SC_POS, y1: SC_POS, x2: SC_POS, y2: SC_POS, opacity: f32) -> GRAPHIN_RESULT,

  pub gPushClipPath: extern "system" fn(hgfx: HGFX, hpath: HPATH, opacity: f32) -> GRAPHIN_RESULT,

  // pop clip layer previously set by gPushClipBox or gPushClipPath
  pub gPopClip: extern "system" fn(hgfx: HGFX) -> GRAPHIN_RESULT,

  // image painter
  pub imagePaint: extern "system" fn(himg: HIMG, pPainter: ImagePaintFunction, prm: LPVOID) -> GRAPHIN_RESULT, // paint on image using graphics

  // VALUE interface
  pub vWrapGfx: extern "system" fn(hgfx: HGFX, toValue: *mut VALUE) -> GRAPHIN_RESULT,

  pub vWrapImage: extern "system" fn(himg: HIMG, toValue: *mut VALUE) -> GRAPHIN_RESULT,

  pub vWrapPath: extern "system" fn(hpath: HPATH, toValue: *mut VALUE) -> GRAPHIN_RESULT,

  pub vWrapText: extern "system" fn(htext: HTEXT, toValue: *mut VALUE) -> GRAPHIN_RESULT,

  pub vUnWrapGfx: extern "system" fn(fromValue: *const VALUE, phgfx: &mut HGFX) -> GRAPHIN_RESULT,

  pub vUnWrapImage: extern "system" fn(fromValue: *const VALUE, phimg: &mut HIMG) -> GRAPHIN_RESULT,

  pub vUnWrapPath: extern "system" fn(fromValue: *const VALUE, phpath: &mut HPATH) -> GRAPHIN_RESULT,

  pub vUnWrapText: extern "system" fn(fromValue: *const VALUE, phtext: &mut HTEXT) -> GRAPHIN_RESULT,

	// since 4.4.3.20
  pub gFlush: extern "system" fn(hgfx: HGFX) -> GRAPHIN_RESULT,
}

pub type ImageWriteFunction = extern "system" fn(prm: LPVOID, data: LPCBYTE, data_length: UINT);
pub type ImagePaintFunction = extern "system" fn(prm: LPVOID, hgfx: HGFX, width: UINT, height: UINT);
