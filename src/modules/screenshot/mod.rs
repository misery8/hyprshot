use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use gtk::prelude::*;

mod events;
pub mod render;
pub mod state;
mod toolbar;
mod ui;

use self::state::ScreenshotState;
use self::ui::ScreenshotWidgets;
use crate::action::{AppAction, GlobalAction, ScreenshotAction};
use crate::capture::clipboard;
use crate::common::cursor;

pub fn run(app: &gtk::Application) {
    let (tx, rx) = mpsc::channel::<AppAction>();
    let app_handle = app.clone();
    let state = Rc::new(RefCell::new(ScreenshotState::default()));

    let widgets = ScreenshotWidgets::build(app, tx.clone(), state.clone());
    let widgets = Rc::new(widgets);

    crate::modules::screenshot::events::init_events(tx, &widgets);

    glib::idle_add_local(move || {
        while let Ok(action) = rx.try_recv() {
            handle_action(&app_handle, action, &state, &widgets);
        }
        glib::ControlFlow::Continue
    });
}

fn handle_action(
    app: &gtk::Application,
    action: AppAction,
    state: &Rc<RefCell<ScreenshotState>>,
    widgets: &ScreenshotWidgets,
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
                        // if let Some(fixed) = widgets.toolbar.widget().parent().and_downcast_ref::<gtk::Fixed>() {
                        //     fixed.show();
                        // }
                        // widgets.toolbar.widget().set_visible(s.is_paused());
                    }
                }
                ScreenshotAction::MouseMove(x, y) => {
                    s.set_mouse_pos((x, y));
                    if s.selection().is_active() {
                        cursor::update_cursor(
                            &s.selection().rect(),
                            s.mouse_pos(),
                            &widgets.canvas,
                        );
                        if s.is_paused() {
                            widgets.toolbar.update_position(&s.selection().rect());
                        }
                        // widgets.toolbar.update_position(&s.selection().rect());
                    }
                }
                ScreenshotAction::DragBegin(x, y) => s.begin_drag(x, y),
                ScreenshotAction::DragUpdate(x, y) => {
                    s.update_drag(x, y);
                    widgets.toolbar.update_position(&s.selection().rect());
                }
                ScreenshotAction::DragEnd => {
                    s.end_drag();

                    if !s.is_paused() {
                        let buf = s.export_selection(&widgets.surface)
                            .expect("Failed export");
                        let _ = clipboard::copy_to_clipboard(&buf);

                        app.quit();
                    }
                }
                ScreenshotAction::Save => {
                    let buf = s.export_selection(&widgets.surface)
                        .expect("Failed export");
                    let _ = clipboard::copy_to_clipboard(&buf);
                    app.quit();
                }
            }
            need_redraw = true;
        }
    }

    if need_redraw {
        widgets.canvas.queue_draw();
    }
}
