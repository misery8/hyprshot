use std::{cell::RefCell, rc::Rc, result::Result};

use anyhow::Error;
use cairo::{Context, ImageSurface};
use gdk4::ffi::gdk_cairo_set_source_pixbuf;
use glib::translate::ToGlibPtr;

use crate::modules::screenshot::{
    render,
    state::{Rect, Shape},
};

const STROKE_PADDING: i32 = 2;
const ARROW_PADDING: i32 = 16;

#[derive(Debug)]
struct HistoryEntry {
    rect: Rect,
    surface: ImageSurface,
}

#[cfg(test)]
impl HistoryEntry {
    fn width(&self) -> i32 {
        self.surface.width()
    }

    fn height(&self) -> i32 {
        self.surface.height()
    }
}

#[derive(Debug)]
pub struct Canvas {
    pub surface: Rc<RefCell<ImageSurface>>,
    history: RefCell<Vec<HistoryEntry>>,
}

impl Canvas {
    pub fn from_screenshot() -> Result<Self, Error> {
        let surface = Rc::new(RefCell::new(Self::prepare_background_surface()));
        let history = RefCell::new(Vec::new());

        Ok(Self { surface, history })
    }

    fn prepare_background_surface() -> ImageSurface {
        let pixbuf = crate::capture::screenshot::capture::capture_fullscreen()
            .expect("Failed to capture screen");

        let surface = ImageSurface::create(cairo::Format::ARgb32, pixbuf.width(), pixbuf.height())
            .expect("Failed to create surface");

        {
            let cr = Context::new(&surface).expect("Failed to create Cairo context");
            unsafe {
                gdk_cairo_set_source_pixbuf(cr.to_raw_none(), pixbuf.to_glib_none().0, 0.0, 0.0);
            }
            cr.paint().expect("Failed to paint pixbuf onto surface");
        }

        surface
    }

    pub fn get_screen_size(&self) -> (i32, i32) {
        let surface = self.surface.borrow();
        (surface.width(), surface.height())
    }

    pub fn restore_snapshot(&self) {
        let Some(entry) = self.history.borrow_mut().pop() else {
            return;
        };

        let surface = self.surface.borrow_mut();
        let cr = Context::new(&*surface).expect("Failed to create undo context");

        cr.save().expect("Failed to save undo context");
        cr.rectangle(
            entry.rect.x as f64,
            entry.rect.y as f64,
            entry.rect.w as f64,
            entry.rect.h as f64,
        );
        cr.clip();
        cr.set_operator(cairo::Operator::Source);
        cr.set_source_surface(&entry.surface, entry.rect.x as f64, entry.rect.y as f64)
            .expect("Failed to set undo surface");
        cr.paint().expect("Failed to restore undo region");
        cr.restore().expect("Failed to restore undo context");
    }

    pub fn apply_shape(&self, shape: &Shape, clip: &Rect) {
        if !shape.is_valid() {
            return;
        }

        let surface = self.surface.borrow_mut();
        let Some(rect) = Self::shape_bounds(shape, &surface) else {
            return;
        };

        if let Result::Ok(snapshot) = Self::clone_region(&surface, rect) {
            self.history.borrow_mut().push(HistoryEntry {
                rect,
                surface: snapshot,
            });
        }

        let cr = Context::new(&*surface).expect("Failed to create bake context");
        render::draw_shape_clipped(&surface, &cr, shape, clip);
    }

    fn clone_region(surface: &ImageSurface, rect: Rect) -> Result<ImageSurface, Error> {
        let copy = ImageSurface::create(surface.format(), rect.w, rect.h)?;
        let cr = Context::new(&copy)?;
        cr.set_operator(cairo::Operator::Source);
        cr.set_source_surface(surface, -rect.x as f64, -rect.y as f64)?;
        cr.paint()?;

        Ok(copy)
    }

    fn shape_bounds(shape: &Shape, surface: &ImageSurface) -> Option<Rect> {
        let surface_w = surface.width();
        let surface_h = surface.height();

        let rect = match shape {
            Shape::Rectangle { rect, .. } => Self::expand_rect(*rect, STROKE_PADDING),
            Shape::Blur { rect } => *rect,
            Shape::Arrow { from, to, .. } => {
                let left = from.0.min(to.0) - ARROW_PADDING;
                let top = from.1.min(to.1) - ARROW_PADDING;
                let right = from.0.max(to.0) + ARROW_PADDING + 1;
                let bottom = from.1.max(to.1) + ARROW_PADDING + 1;

                Rect {
                    x: left,
                    y: top,
                    w: right - left,
                    h: bottom - top,
                }
            }
            Shape::Text { position, runs } => {
                let cr = Context::new(surface).ok()?;
                render::text_bounds(&cr, *position, runs)?
            }
        };

        Self::clamp_rect(rect, surface_w, surface_h)
    }

    fn expand_rect(rect: Rect, padding: i32) -> Rect {
        Rect {
            x: rect.x - padding,
            y: rect.y - padding,
            w: rect.w + padding * 2,
            h: rect.h + padding * 2,
        }
    }

    fn clamp_rect(rect: Rect, surface_w: i32, surface_h: i32) -> Option<Rect> {
        if surface_w <= 0 || surface_h <= 0 {
            return None;
        }

        let left = rect.x.clamp(0, surface_w);
        let top = rect.y.clamp(0, surface_h);
        let right = (rect.x + rect.w).clamp(0, surface_w);
        let bottom = (rect.y + rect.h).clamp(0, surface_h);

        if right <= left || bottom <= top {
            return None;
        }

        Some(Rect {
            x: left,
            y: top,
            w: right - left,
            h: bottom - top,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::screenshot::state::TextRun;

    fn test_canvas(width: i32, height: i32) -> Canvas {
        let surface = ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
        let cr = Context::new(&surface).unwrap();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint().unwrap();

        Canvas {
            surface: Rc::new(RefCell::new(surface)),
            history: RefCell::new(Vec::new()),
        }
    }

    fn surface_png(canvas: &Canvas) -> Vec<u8> {
        let surface = canvas.surface.borrow();
        let mut bytes = Vec::new();
        surface.write_to_png(&mut bytes).unwrap();
        bytes
    }

    fn valid_rectangle() -> Shape {
        Shape::Rectangle {
            rect: Rect {
                x: 20,
                y: 20,
                w: 20,
                h: 20,
            },
            color: (0, 0, 0),
        }
    }

    fn valid_text() -> Shape {
        Shape::Text {
            position: (20, 40),
            runs: vec![TextRun::new("Hello".to_string(), (0, 0, 0))],
        }
    }

    #[test]
    fn applying_valid_shape_creates_undo_entry() {
        let canvas = test_canvas(100, 100);

        canvas.apply_shape(
            &valid_rectangle(),
            &Rect {
                x: 0,
                y: 0,
                w: 100,
                h: 100,
            },
        );

        assert_eq!(canvas.history.borrow().len(), 1);
    }

    #[test]
    fn undo_restores_surface_pixels() {
        let canvas = test_canvas(100, 100);
        let before = surface_png(&canvas);

        canvas.apply_shape(
            &valid_rectangle(),
            &Rect {
                x: 0,
                y: 0,
                w: 100,
                h: 100,
            },
        );
        let after = surface_png(&canvas);
        assert_ne!(after, before);

        canvas.restore_snapshot();

        assert_eq!(surface_png(&canvas), before);
    }

    #[test]
    fn undo_snapshot_is_smaller_than_canvas_for_local_shape() {
        let canvas = test_canvas(100, 100);

        canvas.apply_shape(
            &valid_rectangle(),
            &Rect {
                x: 0,
                y: 0,
                w: 100,
                h: 100,
            },
        );

        let history = canvas.history.borrow();
        let snapshot = &history[0];
        assert!(snapshot.width() < 100);
        assert!(snapshot.height() < 100);
    }

    #[test]
    fn applying_shape_does_not_modify_pixels_outside_selection() {
        let canvas = test_canvas(100, 100);
        let before = surface_png(&canvas);
        let selection = Rect {
            x: 20,
            y: 20,
            w: 40,
            h: 40,
        };
        let shape = Shape::Rectangle {
            rect: Rect {
                x: 30,
                y: 30,
                w: 50,
                h: 50,
            },
            color: (0, 0, 0),
        };

        canvas.apply_shape(&shape, &selection);

        let surface = canvas.surface.borrow();
        let outside = ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();
        {
            let cr = Context::new(&outside).unwrap();
            cr.set_source_surface(&*surface, 0.0, 0.0).unwrap();
            cr.paint().unwrap();
        }
        drop(surface);

        let after = {
            let mut bytes = Vec::new();
            outside.write_to_png(&mut bytes).unwrap();
            bytes
        };

        // The whole image changes because the in-selection part is drawn, but
        // pixels outside the selection must remain identical. Verify by restoring
        // only the selected region from the original and comparing full images.
        {
            let current = canvas.surface.borrow_mut();
            let original = ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();
            let cr = Context::new(&original).unwrap();
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.paint().unwrap();

            let cr = Context::new(&*current).unwrap();
            cr.save().unwrap();
            cr.rectangle(
                selection.x as f64,
                selection.y as f64,
                selection.w as f64,
                selection.h as f64,
            );
            cr.clip();
            cr.set_operator(cairo::Operator::Source);
            cr.set_source_surface(&original, 0.0, 0.0).unwrap();
            cr.paint().unwrap();
            cr.restore().unwrap();
        }

        assert_eq!(surface_png(&canvas), before);
        assert_ne!(after, before);
    }

    #[test]
    fn overflowing_arrow_does_not_modify_pixels_outside_selection() {
        let canvas = test_canvas(100, 100);
        let before = surface_png(&canvas);
        let selection = Rect {
            x: 20,
            y: 20,
            w: 40,
            h: 40,
        };
        let shape = Shape::Arrow {
            from: (30, 30),
            to: (90, 90),
            color: (0, 0, 0),
        };

        canvas.apply_shape(&shape, &selection);
        let after = surface_png(&canvas);
        assert_ne!(after, before);

        {
            let current = canvas.surface.borrow_mut();
            let original = ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();
            let cr = Context::new(&original).unwrap();
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.paint().unwrap();

            let cr = Context::new(&*current).unwrap();
            cr.save().unwrap();
            cr.rectangle(
                selection.x as f64,
                selection.y as f64,
                selection.w as f64,
                selection.h as f64,
            );
            cr.clip();
            cr.set_operator(cairo::Operator::Source);
            cr.set_source_surface(&original, 0.0, 0.0).unwrap();
            cr.paint().unwrap();
            cr.restore().unwrap();
        }

        assert_eq!(surface_png(&canvas), before);
    }

    #[test]
    fn text_shape_uses_regional_undo_snapshot() {
        let canvas = test_canvas(200, 100);

        canvas.apply_shape(
            &valid_text(),
            &Rect {
                x: 0,
                y: 0,
                w: 200,
                h: 100,
            },
        );

        let history = canvas.history.borrow();
        assert_eq!(history.len(), 1);
        assert!(history[0].width() < 200);
        assert!(history[0].height() < 100);
    }
}
