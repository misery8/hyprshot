use cairo::{Context, Format, ImageSurface};

use crate::modules::screenshot::state::ScreenshotState;

pub fn export_selection(original: &ImageSurface, state: &ScreenshotState) -> anyhow::Result<Vec<u8>> {

    let rect = state.selection().rect();
    
    let cropped = ImageSurface::create(Format::ARgb32, rect.w, rect.h)?;
    let cr = Context::new(&cropped)?;

    cr.set_source_surface(original, -rect.x as f64, -rect.y as f64)?;
    cr.paint()?;

    let mut buf = Vec::new();
    cropped.write_to_png(&mut buf)?;
    Ok(buf)
}