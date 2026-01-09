use std::{cell::RefCell, rc::Rc, sync::mpsc::Sender};

use cairo::{Context, ImageSurface};
use gdk::ffi::gdk_cairo_set_source_pixbuf;
use glib::translate::ToGlibPtr;
use gtk::{Application, ApplicationWindow, DrawingArea, Overlay};
use gtk::prelude::*;

use crate::action::AppAction;
use crate::modules::screenshot::state::ScreenshotState;
use crate::modules::screenshot::toolbar::Toolbar;
use crate::modules::screenshot::render;


pub struct ScreenshotWidgets {
    pub window: ApplicationWindow,
    pub canvas: gtk::DrawingArea,
    pub toolbar: Toolbar,
    pub surface: Rc<ImageSurface>
}

impl ScreenshotWidgets {
    pub fn build (
        app: &Application,
        tx: Sender<AppAction>,
        state: Rc<RefCell<ScreenshotState>>
    ) -> Self {        

        let surface= Rc::new(Self::prepare_background_surface());

        // Widgets
        let canvas = DrawingArea::builder()
            .content_width(surface.width())
            .content_height(surface.height())
            .build();

        let toolbar = Toolbar::new(tx);
        let overlay = Self::setup_layout(&canvas, toolbar.widget());

        let window = ApplicationWindow::builder()
            .application(app)
            .child(&overlay)
            .build();

        setup_render_loop(&canvas, state, surface.clone());

        window.fullscreen();
        window.present();

        Self { window, canvas, toolbar, surface }
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

    fn setup_layout(canvas: &DrawingArea, toolbar_widget: &gtk::Box) -> Overlay {

        let overlay = Overlay::new();
        overlay.set_child(Some(canvas));
        overlay.add_overlay(toolbar_widget);

        overlay
    }

}

fn setup_render_loop(
    area: &DrawingArea,
    state: Rc<RefCell<ScreenshotState>>,
    surface: Rc<ImageSurface>
) {
    area.set_draw_func(move |area, cr, _, _| {
        let state = state.borrow();
        
        cr.set_source_surface(&*surface, 0.0, 0.0).unwrap();
        cr.paint().unwrap();

        cr.set_source_rgba(0.0, 0.0, 0.0, 0.6);
        cr.rectangle(0.0, 0.0, area.width() as f64, area.height() as f64);

        if state.selection().is_active() {            
            render::draw_selection(cr, state.selection().rect());
        }

        cr.fill().unwrap();

        if let Some(current_shape) = state.current_shape() {
            render::draw_shape(&surface, cr, current_shape);
        }

        for shape in state.shapes() {
            render::draw_shape(&surface, cr, shape);
        }
    });
} 