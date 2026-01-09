use cairo::{Context, Format, ImageSurface};

use crate::modules::screenshot::state::{ScreenshotState, Shape, Rect};
use crate::modules::screenshot::render;

pub fn export_selection(original: &ImageSurface, state: &ScreenshotState) -> anyhow::Result<Vec<u8>> {

    let rect = state.selection().rect();
    
    let cropped = ImageSurface::create(Format::ARgb32, rect.w, rect.h)?;
    let cr = Context::new(&cropped)?;

    cr.set_source_surface(original, -rect.x as f64, -rect.y as f64)?;
    cr.paint()?;

    for shape in state.shapes() {
        match shape {
            Shape::Arrow { from, to, color } => {
                let local_from = (from.0 - rect.x, from.1 - rect.y);
                let local_to = (to.0 - rect.x, to.1 - rect.y);
                render::draw_arrow(&cr, local_from, local_to, *color);
            }

            Shape::Rectangle { rect: shape_rect, color } => {
                let local_rect = Rect {
                    x: shape_rect.x - rect.x,
                    y: shape_rect.y - rect.y,
                    w: shape_rect.w,
                    h: shape_rect.h,
                };

                render::draw_rectangle(&cr, &local_rect, *color);
            }

            Shape::Blur { rect: shape_rect } => {
                let local_rect = Rect {
                    x: shape_rect.x - rect.x,
                    y: shape_rect.y - rect.y,
                    w: shape_rect.w,
                    h: shape_rect.h,
                };

                render::draw_blur(&cropped, &cr, &local_rect);
            }
        }
    }

    let mut buf = Vec::new();
    cropped.write_to_png(&mut buf)?;
    Ok(buf)
}