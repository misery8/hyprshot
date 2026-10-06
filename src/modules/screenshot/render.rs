use cairo::{Context, ImageSurface};

use crate::modules::screenshot::state::{Rect, Shape};

const ARROW_LINE_WIDTH: f64 = 2.75;
const RECTANGLE_LINE_WIDTH: f64 = 2.25;
const TEXT_FONT_SIZE: f64 = 22.0;
const TEXT_BOUNDS_PADDING: i32 = 4;

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

pub fn draw_selection(
    cr: &Context,
    rect: &Rect,
) {
    let (x, y, w, h) = rect.as_f64();

    cr.rectangle(x, y, w, h);
    cr.set_fill_rule(cairo::FillRule::EvenOdd);

    cr.fill().expect("Cairo fill failed");

    cr.set_operator(cairo::Operator::Over);

    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    cr.set_line_width(1.0);
    cr.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
    cr.stroke().expect("Cairo stroke failed");
}

pub fn draw_shape(surface: &ImageSurface, cr: &Context, shape: &Shape) {
    match shape {
        Shape::Arrow { from, to, color } => draw_arrow(cr, *from, *to, *color),
        Shape::Rectangle { rect, color } => draw_rectangle(cr, rect, *color),
        Shape::Text { position, text, color } => draw_text(cr, *position, text, *color),
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
    text: &str,
    color: (u8, u8, u8),
) {
    if text.is_empty() {
        return;
    }

    set_color(cr, color, 1.0);
    configure_text_font(cr);
    cr.move_to(position.0 as f64, position.1 as f64);
    cr.show_text(text).expect("Cairo text render failed");
}

pub fn draw_text_preview(
    cr: &Context,
    position: (i32, i32),
    text: &str,
    color: (u8, u8, u8),
) {
    draw_text(cr, position, text, color);

    configure_text_font(cr);
    let advance = cr.text_extents(text)
        .map(|extents| extents.x_advance())
        .unwrap_or(0.0);

    set_color(cr, color, 1.0);
    cr.set_line_width(1.5);
    let caret_x = position.0 as f64 + advance + 1.0;
    cr.move_to(caret_x, position.1 as f64 - TEXT_FONT_SIZE);
    cr.line_to(caret_x, position.1 as f64 + 3.0);
    cr.stroke().expect("Cairo caret render failed");
}

pub fn text_bounds(cr: &Context, position: (i32, i32), text: &str) -> Option<Rect> {
    if text.is_empty() {
        return None;
    }

    configure_text_font(cr);
    let extents = cr.text_extents(text).ok()?;

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
