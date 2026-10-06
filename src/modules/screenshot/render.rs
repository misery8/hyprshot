use cairo::{Context, ImageSurface};

use crate::modules::screenshot::state::{Rect, Shape, TextRun};

const ARROW_LINE_WIDTH: f64 = 2.75;
const RECTANGLE_LINE_WIDTH: f64 = 2.25;
const TEXT_FONT_SIZE: f64 = 22.0;
const TEXT_BOUNDS_PADDING: i32 = 4;
const CARET_OUTLINE_WIDTH: f64 = 3.5;
const CARET_INNER_WIDTH: f64 = 1.5;
const SELECTION_DIM_ALPHA: f64 = 0.6;
const SELECTION_LINE_WIDTH: f64 = 1.5;
const SELECTION_DASH: [f64; 2] = [6.0, 6.0];
const SELECTION_HANDLE_SIZE: f64 = 8.0;

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

pub fn draw_selection(
    cr: &Context,
    rect: &Rect,
    viewport: (f64, f64),
) {
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

pub fn draw_arrow(
    cr: &Context,
    from: (i32, i32),
    to: (i32, i32),
    color: (u8, u8, u8),
) {
    let (x1, y1) = (from.0 as f64, from.1 as f64);
    let (x2, y2) = (to.0 as f64, to.1 as f64);

    set_color(cr, color, 1.0);
    cr.set_line_width(ARROW_LINE_WIDTH);

    cr.move_to(x1, y1);
    cr.line_to(x2, y2);
    cr.stroke().expect("Cairo stroke failed");

    let angle = (y2 - y1).atan2(x2 - x1);
    let arrow_len = 16.0;
    let arrow_angle = std::f64::consts::PI / 6.0;

    cr.move_to(x2, y2);
    cr.line_to(
        x2 - arrow_len * (angle - arrow_angle).cos(),
        y2 - arrow_len * (angle - arrow_angle).sin(),
    );
    cr.move_to(x2, y2);
    cr.line_to(
        x2 - arrow_len * (angle + arrow_angle).cos(),
        y2 - arrow_len * (angle + arrow_angle).sin(),
    );
    cr.stroke().expect("Cairo stroke failed");
}

pub fn draw_rectangle(
    cr: &Context,
    rect: &Rect,
    color: (u8, u8, u8)
) {
    let (x, y, w, h) = rect.as_f64();

    set_color(cr, color, 1.0);

    cr.set_line_width(RECTANGLE_LINE_WIDTH);
    cr.rectangle(x, y, w, h);
    cr.stroke().expect("Cairo stroke failed");
}

pub fn draw_text(
    cr: &Context,
    position: (i32, i32),
    runs: &[TextRun],
) {
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

        let advance = cr.text_extents(run.text())
            .map(|extents| extents.x_advance())
            .unwrap_or(0.0);

        set_color(cr, run.color(), 1.0);
        cr.move_to(x, y);
        cr.show_text(run.text()).expect("Cairo text render failed");
        x += advance;
    }
}

fn text_advance(cr: &Context, runs: &[TextRun]) -> f64 {
    configure_text_font(cr);
    runs.iter()
        .filter_map(|run| cr.text_extents(run.text()).ok())
        .map(|extents| extents.x_advance())
        .sum()
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
    let mut text = String::new();
    for run in runs {
        text.push_str(run.text());
    }

    if text.is_empty() {
        return None;
    }

    configure_text_font(cr);
    let extents = cr.text_extents(&text).ok()?;

    let left = (position.0 as f64 + extents.x_bearing()).floor() as i32 - TEXT_BOUNDS_PADDING;
    let top = (position.1 as f64 + extents.y_bearing()).floor() as i32 - TEXT_BOUNDS_PADDING;
    let right = (position.0 as f64 + extents.x_bearing() + extents.width()).ceil() as i32
        + TEXT_BOUNDS_PADDING;
    let bottom = (position.1 as f64 + extents.y_bearing() + extents.height()).ceil() as i32
        + TEXT_BOUNDS_PADDING;

    Some(Rect {
        x: left,
        y: top,
        w: (right - left).max(1),
        h: (bottom - top).max(1),
    })
}

pub fn draw_blur(surface: &ImageSurface, cr: &Context, rect: &Rect) {
    let (x, y, w, h) = rect.as_f64();

    let blurred_region = match crate::common::cairo_blur::blur_image_surface(
        surface, x, y, rect.w, rect.h, 10
    ) {
        Ok(s) => s,
        Err(_) => return,
    };

    cr.save().expect("Failed to save state");
    cr.rectangle(x, y, w, h);
    cr.clip();

    cr.set_source_surface(&blurred_region, x, y).expect("Failed to set source");
    cr.paint().expect("Failed to paint");

    cr.restore().expect("Failed to restore state");
}


#[cfg(test)]
mod tests {
    use super::*;

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
