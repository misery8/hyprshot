use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use gtk::prelude::*;

mod canvas;
mod events;
mod render;
pub mod state;
mod toolbar;
mod ui;

use self::state::ScreenshotState;
use self::ui::ScreenshotWidgets;
use crate::action::{AppAction, GlobalAction, ScreenshotAction};
use crate::capture::clipboard;
use crate::common::cursor;
use crate::modules::screenshot::canvas::Canvas;

pub fn run(app: &gtk::Application) {
    let (tx, rx) = mpsc::channel::<AppAction>();
    let app_handle = app.clone();
    let state = Rc::new(RefCell::new(ScreenshotState::default()));

    let canvas = Rc::new(
        Canvas::from_screenshot()
            .expect("Failed to create ")
    );

    let widgets = Rc::new(
        ScreenshotWidgets::build(
            app,
            tx.clone(),
            state.clone(),
            canvas.clone(),
        )
    );

    crate::modules::screenshot::events::init_events(tx, &widgets);

    glib::idle_add_local(move || {
        while let Ok(action) = rx.try_recv() {
            handle_action(&app_handle, action, &state, &widgets, &canvas);
        }
        glib::ControlFlow::Continue
    });
}

fn handle_action(
    app: &gtk::Application,
    action: AppAction,
    state: &Rc<RefCell<ScreenshotState>>,
    widgets: &ScreenshotWidgets,
    canvas: &Canvas,
) {
    let mut s = state.borrow_mut();
    let mut need_redraw = false;

    match action {
        AppAction::Global(GlobalAction::Quit) => app.quit(),
        AppAction::Screenshot(sub_action) => {
            match sub_action {
                ScreenshotAction::SetTool(tool) => s.set_tool(tool),
                ScreenshotAction::SetColor(red, green, blue) => s.set_color((red, green, blue)),
                ScreenshotAction::ToogleMode => {
                    if s.selection().is_active() && !s.is_paused() {
                        s.toogle_pause();
                        widgets.toolbar.widget().set_opacity(1.0);
                    }
                }
                ScreenshotAction::MouseMove(x, y) => {
                    s.set_mouse_pos((x, y));
                    if s.selection().is_active() {
                        cursor::update_cursor(
                            &s.selection().rect(),
                            s.mouse_pos(),
                            &widgets.drawing_area,
                        );
                        if s.is_paused() {
                            widgets.toolbar.update_position(&s.selection().rect());
                        }
                    }
                }
                ScreenshotAction::DragBegin(x, y) => { 
                    s.begin_drag(x, y);
                    canvas.save_shapshot(&*s);
                },
                ScreenshotAction::DragUpdate(x, y) => {
                    s.update_drag(x, y);
                    widgets.toolbar.update_position(&s.selection().rect());
                }
                ScreenshotAction::DragEnd => {
                    if let Some(shape) = s.current_shape() {
                        canvas.apply_shape(shape);
                    }
                    s.end_drag();

                    if !s.is_paused() {
                        let buf = s.export_selection(&canvas.surface.borrow())
                            .expect("Failed export");
                        let _ = clipboard::copy_to_clipboard(&buf);

                        app.quit();
                    }
                }
                ScreenshotAction::Save => {
                    let buf = s.export_selection(&canvas.surface.borrow())
                        .expect("Failed export");
                    let _ = clipboard::copy_to_clipboard(&buf);
                    app.quit();
                }

                ScreenshotAction::Undo => { canvas.restore_snapshot() }

            }
            need_redraw = true;
        }
    }

    if need_redraw {
        widgets.drawing_area.queue_draw();
    }
}
