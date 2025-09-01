use hbb_common::{bail, ResultType};
use tiny_skia::{FillRule, Paint, PathBuilder, PixmapMut, Point, Transform};
use ttf_parser::Face;
// A helper struct to bridge `ttf-parser` and `tiny-skia`.
struct PathBuilderWrapper<'a> {
    path_builder: &'a mut PathBuilder,
    transform: Transform,
}

impl ttf_parser::OutlineBuilder for PathBuilderWrapper<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let mut pt = Point::from_xy(x, y);
        self.transform.map_point(&mut pt);
        self.path_builder.move_to(pt.x, pt.y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let mut pt = Point::from_xy(x, y);
        self.transform.map_point(&mut pt);
        self.path_builder.line_to(pt.x, pt.y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let mut pt1 = Point::from_xy(x1, y1);
        self.transform.map_point(&mut pt1);
        let mut pt = Point::from_xy(x, y);
        self.transform.map_point(&mut pt);
        self.path_builder.quad_to(pt1.x, pt1.y, pt.x, pt.y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let mut pt1 = Point::from_xy(x1, y1);
        self.transform.map_point(&mut pt1);
        let mut pt2 = Point::from_xy(x2, y2);
        self.transform.map_point(&mut pt2);
        let mut pt = Point::from_xy(x, y);
        self.transform.map_point(&mut pt);
        self.path_builder
            .cubic_to(pt1.x, pt1.y, pt2.x, pt2.y, pt.x, pt.y);
    }

    fn close(&mut self) {
        self.path_builder.close();
    }
}

// Draws a string of text onto the pixmap.
pub(super) fn draw_text(
    pixmap: &mut PixmapMut,
    face: &Face,
    text: &str,
    x: f32,
    y: f32,
    paint: &Paint,
    font_size: f32,
) {
    let units_per_em = face.units_per_em() as f32;
    let scale = font_size / units_per_em;
    let transform = Transform::from_translate(x, y).pre_scale(scale, -scale);

    let mut path_builder = PathBuilder::new();
    let mut current_x = 0.0;

    for ch in text.chars() {
        let glyph_id = face.glyph_index(ch).unwrap_or_default();

        let mut builder = PathBuilderWrapper {
            path_builder: &mut path_builder,
            transform: transform.post_translate(current_x, 0.0),
        };

        face.outline_glyph(glyph_id, &mut builder);

        if let Some(h_advance) = face.glyph_hor_advance(glyph_id) {
            current_x += h_advance as f32 * scale;
        }
    }

    if let Some(path) = path_builder.finish() {
        pixmap.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
    }
}

pub(super) fn create_font_face() -> ResultType<Face<'static>> {
    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();
    let query = fontdb::Query {
        families: &[fontdb::Family::Monospace],
        ..fontdb::Query::default()
    };
    let Some(font_id) = font_db.query(&query) else {
        bail!("No monospace font found!");
    };
    let Some((font_source, face_index)) = font_db.face_source(font_id) else {
        bail!("No face found for font!");
    };
    let font_data: &'static [u8] = Box::leak(match font_source {
        fontdb::Source::File(path) => std::fs::read(path)?.into_boxed_slice(),
        fontdb::Source::Binary(data) => data.as_ref().as_ref().to_vec().into_boxed_slice(),
        fontdb::Source::SharedFile(path, _) => std::fs::read(path)?.into_boxed_slice(),
    });
    let face = Face::parse(font_data, face_index)?;
    Ok(face)
}
