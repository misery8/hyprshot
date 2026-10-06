use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc::Sender,
    time::{Duration, Instant},
};

use gtk4::{Application, ApplicationWindow, DrawingArea, Overlay};
use gtk4::prelude::*;
use gtk4_layer_shell::LayerShell;

use crate::action::AppAction;
use crate::modules::screenshot::canvas::Canvas;
use crate::modules::screenshot::render;
use crate::modules::screenshot::state::ScreenshotState;
use crate::modules::screenshot::toolbar::Toolbar;

const CARET_BLINK_INTERVAL: Duration = Duration::from_millis(500);

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

        // let da_size = {
        //     let surf = canvas.surface.borrow();
        //     (surf.width(), surf.height())
        // };

        let drawing_area = DrawingArea::builder()
            // .content_width(da_size.0)
            // .content_height(da_size.1)
            .hexpand(true)
            .vexpand(true)
            .build();

        Self::setup_render_loop(&drawing_area, state, canvas);
        
        let toolbar = Toolbar::new(tx);
        let overlay = Self::setup_layout(&drawing_area, toolbar.widget());

        let window = ApplicationWindow::builder()
            .application(app)
            .child(&overlay)
            .title("Hyprshot")
            .decorated(false)
            .build();
        
        window.init_layer_shell();
        window.set_layer(gtk4_layer_shell::Layer::Overlay);
        window.set_exclusive_zone(-1);
        window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::OnDemand);

        window.set_anchor(gtk4_layer_shell::Edge::Top, true);
        window.set_anchor(gtk4_layer_shell::Edge::Bottom, true);
        window.set_anchor(gtk4_layer_shell::Edge::Left, true);
        window.set_anchor(gtk4_layer_shell::Edge::Right, true);

        window.present();

        Self { window, drawing_area, toolbar }
    }

    fn setup_layout(da: &DrawingArea, toolbar_widget: &gtk4::Box) -> Overlay {

        let overlay = Overlay::new();
        overlay.set_vexpand(true);
        overlay.set_hexpand(true);
        overlay.set_child(Some(da));
        overlay.add_overlay(toolbar_widget);

        overlay
    }

    fn setup_render_loop(
        da: &DrawingArea,
        state: Rc<RefCell<ScreenshotState>>,
        canvas: Rc<Canvas>,
    ) {
        let caret_visible = Rc::new(Cell::new(true));
        let last_text_len = Rc::new(Cell::new(None::<usize>));
        let last_caret_toggle = Rc::new(RefCell::new(Instant::now()));

        let tick_state = state.clone();
        let tick_caret_visible = caret_visible.clone();
        let tick_last_text_len = last_text_len.clone();
        let tick_last_caret_toggle = last_caret_toggle.clone();

        da.add_tick_callback(move |area, _| {
            let text_len = tick_state
                .borrow()
                .text_input()
                .map(|input| input.text().len());

            match text_len {
                Some(len) => {
                    if tick_last_text_len.get() != Some(len) {
                        tick_last_text_len.set(Some(len));
                        tick_caret_visible.set(true);
                        *tick_last_caret_toggle.borrow_mut() = Instant::now();
                        area.queue_draw();
                    } else if tick_last_caret_toggle.borrow().elapsed() >= CARET_BLINK_INTERVAL {
                        tick_caret_visible.set(!tick_caret_visible.get());
                        *tick_last_caret_toggle.borrow_mut() = Instant::now();
                        area.queue_draw();
                    }
                }
                None => {
                    tick_last_text_len.set(None);
                    tick_caret_visible.set(true);
                    *tick_last_caret_toggle.borrow_mut() = Instant::now();
                }
            }

            glib::ControlFlow::Continue
        });

        da.set_draw_func(move |area, cr, _, _| {
            let state = state.borrow();
            let surface = canvas.surface.borrow();
            
            cr.set_source_surface(&*surface, 0.0, 0.0).unwrap();
            cr.paint().unwrap();

            if state.selection().is_active() {
                render::draw_selection(
                    cr,
                    state.selection().rect(),
                    (area.width() as f64, area.height() as f64),
                );
            } else {
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.6);
                cr.paint().expect("Cairo dim overlay paint failed");
            }

            if let Some(shape) = state.current_shape() {
                render::draw_shape(&surface, cr, shape);
            }

            if let Some(input) = state.text_input() {
                cr.save().expect("Failed to save text preview context");
                let (x, y, w, h) = state.selection().rect().as_f64();
                cr.rectangle(x, y, w, h);
                cr.clip();

                render::draw_text_preview(
                    cr,
                    input.position(),
                    input.runs(),
                    caret_visible.get(),
                );
                cr.restore().expect("Failed to restore text preview context");
            }
        });
    }

}
