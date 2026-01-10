use std::{cell::RefCell, rc::Rc, sync::mpsc::Sender};

use gtk::{Application, ApplicationWindow, DrawingArea, Overlay};
use gtk::prelude::*;
use gtk4_layer_shell::LayerShell;

use crate::action::AppAction;
use crate::modules::screenshot::canvas::Canvas;
use crate::modules::screenshot::render;
use crate::modules::screenshot::state::ScreenshotState;
use crate::modules::screenshot::toolbar::Toolbar;


pub struct ScreenshotWidgets {
    pub window: ApplicationWindow,
    pub drawing_area: DrawingArea,
    pub toolbar: Toolbar,
}

impl ScreenshotWidgets {
    pub fn build (
        app: &Application,
        tx: Sender<AppAction>,
        state: Rc<RefCell<ScreenshotState>>,
        canvas: Rc<Canvas>,
    ) -> Self {

        let da_size = {
            let surf = canvas.surface.borrow();
            (surf.width(), surf.height())
        };

        let drawing_area = DrawingArea::builder()
            .content_width(da_size.0)
            .content_height(da_size.1)
            .build();

        Self::setup_render_loop(&drawing_area, state, canvas);
        
        let toolbar = Toolbar::new(tx);
        let overlay = Self::setup_layout(&drawing_area, toolbar.widget());

        let window = ApplicationWindow::builder()
            .application(app)
            .child(&overlay)
            .title("Hyprshot")
            .build();
        
        window.init_layer_shell();
        window.set_layer(gtk4_layer_shell::Layer::Overlay);
        window.set_exclusive_zone(-1);
        window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::Exclusive);

        window.set_anchor(gtk4_layer_shell::Edge::Top, true);
        window.set_anchor(gtk4_layer_shell::Edge::Bottom, true);
        window.set_anchor(gtk4_layer_shell::Edge::Left, true);
        window.set_anchor(gtk4_layer_shell::Edge::Right, true);

        window.present();

        Self { window, drawing_area, toolbar }
    }

    fn setup_layout(da: &DrawingArea, toolbar_widget: &gtk::Box) -> Overlay {

        let overlay = Overlay::new();
        overlay.set_child(Some(da));
        overlay.add_overlay(toolbar_widget);

        overlay
    }

    fn setup_render_loop(
        da: &DrawingArea,
        state: Rc<RefCell<ScreenshotState>>,
        canvas: Rc<Canvas>,
    ) {
        da.set_draw_func(move |area, cr, _, _| {
            let state = state.borrow();
            let surface = canvas.surface.borrow();
            
            cr.set_source_surface(&*surface, 0.0, 0.0).unwrap();
            cr.paint().unwrap();

            cr.set_source_rgba(0.0, 0.0, 0.0, 0.6);
            cr.rectangle(0.0, 0.0, area.width() as f64, area.height() as f64);

            if state.selection().is_active() {            
                render::draw_selection(cr, state.selection().rect());
            }

            cr.fill().unwrap();

            if let Some(shape) = state.current_shape() {
                render::draw_shape(&surface, cr, shape);
            }
        });
    }

}

 