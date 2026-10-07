use cairo::{Context, ImageSurface};

use crate::modules::screenshot::state::{Rect, Shape, TextRun};

pub(crate) const ARROW_LINE_WIDTH: f64 = 2.75;
pub(crate) const RECTANGLE_LINE_WIDTH: f64 = 2.25;
pub(crate) const ARROW_HEAD_LENGTH: f64 = 16.0;
pub(crate) const ARROW_HEAD_ANGLE: f64 = std::f64::consts::PI / 6.0;
const TEXT_FONT_SIZE: f64 = 22.0;
const TEXT_BOUNDS_PADDING: i32 = 4;
const CARET_OUTLINE_WIDTH: f64 = 3.5;
const CARET_INNER_WIDTH: f64 = 1.5;
const SELECTION_DIM_ALPHA: f64 = 0.6;
const SELECTION_LINE_WIDTH: f64 = 1.5;
const SELECTION_DASH: [f64; 2] = [6.0, 6.0];
const SELECTION_HANDLE_SIZE: f64 = 8.0;

pub(crate) fn stroke_padding(line_width: f64) -> i32 {
    (line_width / 2.0).ceil() as i32
}

pub(crate) fn arrow_head_offsets(angle: f64) -> [(f64, f64); 2] {
    [
        (
            -ARROW_HEAD_LENGTH * (angle - ARROW_HEAD_ANGLE).cos(),
            -ARROW_HEAD_LENGTH * (angle - ARROW_HEAD_ANGLE).sin(),
        ),
        (
            -ARROW_HEAD_LENGTH * (angle + ARROW_HEAD_ANGLE).cos(),
            -ARROW_HEAD_LENGTH * (angle + ARROW_HEAD_ANGLE).sin(),
        ),
    ]
}

pub(crate) fn arrow_head_points(
    from: (i32, i32),
    to: (i32, i32),
) -> [(f64, f64); 2] {
    let angle = ((to.1 - from.1) as f64).atan2((to.0 - from.0) as f64);
    let offsets = arrow_head_offsets(angle);

    [
        (to.0 as f64 + offsets[0].0, to.1 as f64 + offsets[0].1),
        (to.0 as f64 + offsets[1].0, to.1 as f64 + offsets[1].1),
    ]
}

fn set_color(cr: &Context, color: (u8, u8, u8), alpha: f64) {
    cr.set_source_rgba(
        color.0 as f64 / 255.0,
        color.1 as f64 / 255.0,
        color.2 as f64 / 255.0,
        alpha,
    );
}

fn configure_text_font(cr: &Context) {
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    cr.set_font_size(TEXT_FONT_SIZE);
}

fn selection_handle_positions(rect: &Rect) -> [(f64, f64); 8] {
    let (x, y, w, h) = rect.as_f64();
    let right = x + w;
    let bottom = y + h;
    let center_x = x + w / 2.0;
    let center_y = y + h / 2.0;

    [
        (x, y),
        (center_x, y),
        (right, y),
        (x, center_y),
        (right, center_y),
        (x, bottom),
        (center_x, bottom),
        (right, bottom),
    ]
}

pub fn draw_selection(cr: &Context, rect: &Rect, viewport: (f64, f64)) {
    let (x, y, w, h) = rect.as_f64();

    cr.save().expect("Failed to save selection context");

    cr.set_operator(cairo::Operator::Over);
    cr.set_source_rgba(0.0, 0.0, 0.0, SELECTION_DIM_ALPHA);
    cr.rectangle(0.0, 0.0, viewport.0, viewport.1);
    cr.rectangle(x, y, w, h);
    cr.set_fill_rule(cairo::FillRule::EvenOdd);
    cr.fill().expect("Cairo selection overlay fill failed");

    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    cr.set_line_width(SELECTION_LINE_WIDTH);
    cr.set_dash(&SELECTION_DASH, 0.0);
    cr.rectangle(x, y, w, h);
    cr.stroke().expect("Cairo selection stroke failed");

    let half = SELECTION_HANDLE_SIZE / 2.0;
    for (handle_x, handle_y) in selection_handle_positions(rect) {
        cr.rectangle(
            handle_x - half,
            handle_y - half,
            SELECTION_HANDLE_SIZE,
            SELECTION_HANDLE_SIZE,
        );
        cr.fill().expect("Cairo selection handle fill failed");
    }

    cr.restore().expect("Failed to restore selection context");
}

pub fn draw_shape(surface: &ImageSurface, cr: &Context, shape: &Shape) {
    match shape {
        Shape::Arrow { from, to, color } => draw_arrow(cr, *from, *to, *color),
        Shape::Rectangle { rect, color } => draw_rectangle(cr, rect, *color),
        Shape::Text { position, runs } => draw_text(cr, *position, runs),
        Shape::Blur { rect } => draw_blur(surface, cr, rect),
    }
}

pub fn draw_shape_clipped(
    surface: &ImageSurface,
    cr: &Context,
    shape: &Shape,
    clip: &Rect,
) {
    let (x, y, w, h) = clip.as_f64();

    cr.save().expect("Failed to save annotation clip context");
    cr.rectangle(x, y, w, h);
    cr.clip();
    draw_shape(surface, cr, shape);
    cr.restore().expect("Failed to restore annotation clip context");
}

pub fn draw_arrow(cr: &Context, from: (i32, i32), to: (i32, i32), color: (u8, u8, u8)) {
    let (x1, y1) = (from.0 as f64, from.1 as f64);
    let (x2, y2) = (to.0 as f64, to.1 as f64);

    set_color(cr, color, 1.0);
    cr.set_line_width(ARROW_LINE_WIDTH);

    cr.move_to(x1, y1);
    cr.line_to(x2, y2);
    cr.stroke().expect("Cairo stroke failed");

    let [head_a, head_b] = arrow_head_points(from, to);

    cr.move_to(x2, y2);
    cr.line_to(head_a.0, head_a.1);
    cr.move_to(x2, y2);
    cr.line_to(head_b.0, head_b.1);
    cr.stroke().expect("Cairo stroke failed");
}

pub fn draw_rectangle(cr: &Context, rect: &Rect, color: (u8, u8, u8)) {
    let (x, y, w, h) = rect.as_f64();

    set_color(cr, color, 1.0);

    cr.set_line_width(RECTANGLE_LINE_WIDTH);
    cr.rectangle(x, y, w, h);
    cr.stroke().expect("Cairo stroke failed");
}

pub fn draw_text(cr: &Context, position: (i32, i32), runs: &[TextRun]) {
    if runs.is_empty() {
        return;
    }

    configure_text_font(cr);
    let mut x = position.0 as f64;
    let y = position.1 as f64;

    for run in runs {
        if run.text().is_empty() {
            continue;
        }

        let advance = cr
            .text_extents(run.text())
            .map(|extents| extents.x_advance())
            .unwrap_or(0.0);

        set_color(cr, run.color(), 1.0);
        cr.move_to(x, y);
        cr.show_text(run.text()).expect("Cairo text render failed");
        x += advance;
    }
}

#[derive(Debug, Clone, Copy)]
struct TextLayout {
    ink: Option<(f64, f64, f64, f64)>,
    advance: f64,
}

fn text_layout(cr: &Context, position: (i32, i32), runs: &[TextRun]) -> Option<TextLayout> {
    configure_text_font(cr);

    let mut cursor_x = position.0 as f64;
    let mut ink: Option<(f64, f64, f64, f64)> = None;

    for run in runs {
        if run.text().is_empty() {
            continue;
        }

        let extents = cr.text_extents(run.text()).ok()?;
        if extents.width() > 0.0 && extents.height() > 0.0 {
            let left = cursor_x + extents.x_bearing();
            let top = position.1 as f64 + extents.y_bearing();
            let right = left + extents.width();
            let bottom = top + extents.height();

            ink = Some(match ink {
                Some((ink_left, ink_top, ink_right, ink_bottom)) => (
                    ink_left.min(left),
                    ink_top.min(top),
                    ink_right.max(right),
                    ink_bottom.max(bottom),
                ),
                None => (left, top, right, bottom),
            });
        }

        cursor_x += extents.x_advance();
    }

    Some(TextLayout {
        ink,
        advance: cursor_x - position.0 as f64,
    })
}

fn text_advance(cr: &Context, runs: &[TextRun]) -> f64 {
    text_layout(cr, (0, 0), runs)
        .map(|layout| layout.advance)
        .unwrap_or(0.0)
}

fn draw_caret(cr: &Context, caret_x: f64, baseline_y: f64) {
    let top = baseline_y - TEXT_FONT_SIZE;
    let bottom = baseline_y + 3.0;

    cr.set_source_rgba(0.0, 0.0, 0.0, 0.85);
    cr.set_line_width(CARET_OUTLINE_WIDTH);
    cr.move_to(caret_x, top);
    cr.line_to(caret_x, bottom);
    cr.stroke().expect("Cairo caret outline render failed");

    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    cr.set_line_width(CARET_INNER_WIDTH);
    cr.move_to(caret_x, top);
    cr.line_to(caret_x, bottom);
    cr.stroke().expect("Cairo caret render failed");
}

pub fn draw_text_preview(
    cr: &Context,
    position: (i32, i32),
    runs: &[TextRun],
    caret_visible: bool,
) {
    draw_text(cr, position, runs);

    if !caret_visible {
        return;
    }

    let advance = text_advance(cr, runs);
    let caret_x = position.0 as f64 + advance + 1.0;
    draw_caret(cr, caret_x, position.1 as f64);
}

pub fn text_bounds(cr: &Context, position: (i32, i32), runs: &[TextRun]) -> Option<Rect> {
    let layout = text_layout(cr, position, runs)?;
    let (ink_left, ink_top, ink_right, ink_bottom) = layout.ink?;

    let left = ink_left.floor() as i32 - TEXT_BOUNDS_PADDING;
    let top = ink_top.floor() as i32 - TEXT_BOUNDS_PADDING;
    let right = ink_right.ceil() as i32 + TEXT_BOUNDS_PADDING;
    let bottom = ink_bottom.ceil() as i32 + TEXT_BOUNDS_PADDING;

    Some(Rect {
        x: left,
        y: top,
        w: (right - left).max(1),
        h: (bottom - top).max(1),
    })
}

pub fn text_preview_bounds(
    cr: &Context,
    position: (i32, i32),
    runs: &[TextRun],
) -> Option<Rect> {
    let layout = text_layout(cr, position, runs)?;
    let caret_x = position.0 as f64 + layout.advance + 1.0;
    let caret_half_width = CARET_OUTLINE_WIDTH / 2.0;

    let mut left = (caret_x - caret_half_width).floor() as i32;
    let mut top = (position.1 as f64 - TEXT_FONT_SIZE).floor() as i32;
    let mut right = (caret_x + caret_half_width).ceil() as i32;
    let mut bottom = (position.1 as f64 + 3.0).ceil() as i32;

    if let Some((ink_left, ink_top, ink_right, ink_bottom)) = layout.ink {
        left = left.min(ink_left.floor() as i32 - TEXT_BOUNDS_PADDING);
        top = top.min(ink_top.floor() as i32 - TEXT_BOUNDS_PADDING);
        right = right.max(ink_right.ceil() as i32 + TEXT_BOUNDS_PADDING);
        bottom = bottom.max(ink_bottom.ceil() as i32 + TEXT_BOUNDS_PADDING);
    }

    Some(Rect {
        x: left,
        y: top,
        w: (right - left).max(1),
        h: (bottom - top).max(1),
    })
}

pub fn draw_blur(surface: &ImageSurface, cr: &Context, rect: &Rect) {
    let (x, y, w, h) = rect.as_f64();

    let blurred_region =
        match crate::common::cairo_blur::blur_image_surface(surface, x, y, rect.w, rect.h, 10) {
            Ok(s) => s,
            Err(_) => return,
        };

    cr.save().expect("Failed to save state");
    cr.rectangle(x, y, w, h);
    cr.clip();

    cr.set_source_surface(&blurred_region, x, y)
        .expect("Failed to set source");
    cr.paint().expect("Failed to paint");

    cr.restore().expect("Failed to restore state");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn containment_test_glyph_has_negative_x_bearing() {
        let surface = ImageSurface::create(cairo::Format::ARgb32, 1, 1).unwrap();
        let cr = Context::new(&surface).unwrap();
        configure_text_font(&cr);

        let extents = cr.text_extents("j").unwrap();
        assert!(
            extents.x_bearing() < 0.0,
            "test precondition requires a negative-bearing glyph"
        );
    }

    #[test]
    fn text_bounds_follow_run_by_run_layout() {
        let surface = ImageSurface::create(cairo::Format::ARgb32, 1, 1).unwrap();
        let cr = Context::new(&surface).unwrap();
        configure_text_font(&cr);

        let position = (50, 60);
        let first = cr.text_extents("A").unwrap();
        let second = cr.text_extents("V").unwrap();
        let second_x = position.0 as f64 + first.x_advance();

        let left = (position.0 as f64 + first.x_bearing())
            .min(second_x + second.x_bearing())
            .floor() as i32
            - TEXT_BOUNDS_PADDING;
        let top = (position.1 as f64 + first.y_bearing())
            .min(position.1 as f64 + second.y_bearing())
            .floor() as i32
            - TEXT_BOUNDS_PADDING;
        let right = (position.0 as f64 + first.x_bearing() + first.width())
            .max(second_x + second.x_bearing() + second.width())
            .ceil() as i32
            + TEXT_BOUNDS_PADDING;
        let bottom = (position.1 as f64 + first.y_bearing() + first.height())
            .max(position.1 as f64 + second.y_bearing() + second.height())
            .ceil() as i32
            + TEXT_BOUNDS_PADDING;

        let runs = [
            TextRun::new("A".to_string(), (255, 0, 0)),
            TextRun::new("V".to_string(), (0, 255, 0)),
        ];

        assert_eq!(
            text_bounds(&cr, position, &runs),
            Some(Rect {
                x: left,
                y: top,
                w: right - left,
                h: bottom - top,
            })
        );
    }

    #[test]
    fn selection_handles_match_demo_positions() {
        let rect = Rect { x: 10, y: 20, w: 100, h: 80 };

        assert_eq!(
            selection_handle_positions(&rect),
            [
                (10.0, 20.0),
                (60.0, 20.0),
                (110.0, 20.0),
                (10.0, 60.0),
                (110.0, 60.0),
                (10.0, 100.0),
                (60.0, 100.0),
                (110.0, 100.0),
            ]
        );
    }
}
