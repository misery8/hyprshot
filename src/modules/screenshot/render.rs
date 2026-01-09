use cairo::{Context, ImageSurface};

use crate::modules::screenshot::state::{Rect, Shape};

fn set_color(cr: &Context, color: (u8, u8, u8), alpha: f64) {
    cr.set_source_rgba(
        color.0 as f64 / 255.0,
        color.1 as f64 / 255.0,
        color.2 as f64 / 255.0,
        alpha,
    );
}

pub fn draw_selection(
    cr: &Context,
    rect: &Rect,
) {
    let (x, y, w, h) = rect.as_f64();

    // cr.set_operator(cairo::Operator::Clear);
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
    
    cr.set_line_width(2.5);
    cr.move_to(x1, y1);
    cr.line_to(x2, y2);
    cr.stroke().expect("Cairo stroke failed");

    let angle = (y2 - y1).atan2(x2 - x1);
    let arrow_len = 14.0;
    let arrow_ang = 0.5;

    cr.move_to(x2, y2);
    cr.line_to(
        x2 - arrow_len * (angle - arrow_ang).cos(),
        y2 - arrow_len * (angle - arrow_ang).sin()
    );
    cr.line_to(
        x2 - arrow_len * (angle + arrow_ang).cos(),
        y2 - arrow_len * (angle + arrow_ang).sin()
    );

    cr.close_path();
    cr.fill().expect("Cairo stroke failed");

}

pub fn draw_rectangle(
    cr: &Context,
    rect: &Rect,
    color: (u8, u8, u8)
) {
    let (x, y, w, h) = rect.as_f64();

    set_color(cr, color, 1.0);

    cr.set_line_width(2.0);
    cr.rectangle(x, y, w, h);
    cr.stroke().expect("Cairo stroke failed");
}

pub fn draw_blur(
    surface: &ImageSurface,
    cr: &Context,
    rect: &Rect
) {

    let (x, y, w, h) = rect.as_f64();

    let blur_surface = ImageSurface::create(cairo::Format::ARgb32, rect.w, rect.h)
        .expect("Failed to create temporary surface");

    let blur_cr = Context::new(&blur_surface)
        .expect("Failed to create blur context");
    blur_cr.set_source_surface(surface, -x, -y).expect("Failed to set source");
    blur_cr.paint().expect("Failed to paint copy");
    
    crate::common::cairo_blur::blur_image_surface_v2(&blur_surface, 10)
        .expect("Failed to blur surface");

    cr.save().expect("Failed to save state");
    
    cr.rectangle(x, y, w, h);
    cr.clip();

    cr.set_source_surface(&blur_surface, x, y).expect("Failed to set result source");
    cr.paint().expect("Failed to paint final blur");
    
    cr.restore().expect("Failed to restore state");

}
