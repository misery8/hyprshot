use std::{cell::RefCell, rc::Rc, result::Result};

use anyhow::{Ok, Error};
use gdk::ffi::gdk_cairo_set_source_pixbuf;
use cairo::{ImageSurface, Context};
use glib::translate::ToGlibPtr;

use crate::modules::screenshot::{render, state::{ScreenshotState, Shape, Tool}};

#[derive(Debug, Clone)]
pub struct Canvas {
    pub surface: Rc<RefCell<ImageSurface>>,
    history: RefCell<Vec<ImageSurface>>,
}

impl Canvas {
    pub fn from_screenshot() -> Result<Self, Error> {
        let surface= Rc::new(RefCell::new(Self::prepare_background_surface()));
        let history = RefCell::new(Vec::new());

        Ok(Self { surface, history })
    }

    fn prepare_background_surface() -> ImageSurface {
        let pixbuf = crate::capture::screenshot::capture::capture_fullscreen()
            .expect("Failed to capture screen");

        let surface = ImageSurface::create(cairo::Format::ARgb32, pixbuf.width(), pixbuf.height())
            .expect("Failed to create surface");

        {
            let cr = Context::new(&surface)
                .expect("Failed to create Cairo context");
            unsafe {
                gdk_cairo_set_source_pixbuf(cr.to_raw_none(), pixbuf.to_glib_none().0, 0.0, 0.0);
            }
            cr.paint().expect("Failet to paint pixbuf onto surface");
        }

        surface
    }

    pub fn save_shapshot(&self, state: &ScreenshotState) {

        if state.current_tool() != Tool::None
            && state.is_paused()
        {
            let surface = self.surface.borrow();
            if let Result::Ok(backup) = Self::clone_surface(&surface) {
                self.history.borrow_mut().push(backup);
            }
        }
    }

    pub fn restore_snapshot(&self) {
        if let Some(previus_surface) = self.history.borrow_mut().pop() {
            *self.surface.borrow_mut() = previus_surface;
        }
    }

    pub fn clone_surface(surface: &ImageSurface) -> Result<ImageSurface, Error> {
        let copy = ImageSurface::create(surface.format(), surface.width(), surface.height())?;
        let cr = Context::new(&copy)?;
        cr.set_source_surface(surface, 0.0, 0.0)?;
        cr.paint()?;
        
        Ok(copy)
    }

    pub fn apply_shape(&self, shape: &Shape) {
        
        if shape.is_valid() {
            let surface = self.surface.borrow_mut();
            let cr = Context::new(&*surface)
                .expect("Failed to bake context");
            render::draw_shape(&*surface, &cr, shape);
        } else {
            self.history.borrow_mut().pop();
        }
    }

}