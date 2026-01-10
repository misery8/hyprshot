use std::sync::mpsc::Sender;

use gdk::Key;
use glib::clone;
use gtk::{
    EventControllerMotion, GestureDrag, EventControllerKey,
    Shortcut, CallbackAction, ShortcutController, ShortcutTrigger, prelude::*
};

use crate::action::{AppAction, GlobalAction, ScreenshotAction};
use crate::modules::screenshot::ui::ScreenshotWidgets;

pub fn init_events(tx: Sender<AppAction>, widgets: &ScreenshotWidgets) {

    let drag = GestureDrag::new();
    drag.set_button(1);

    drag.connect_drag_begin(clone!(#[strong] tx, move |_g, x, y| {
            let _ = tx.send(AppAction::Screenshot(ScreenshotAction::DragBegin(x as i32, y as i32)));
        }
    ));

    drag.connect_drag_update(clone!(#[strong] tx, move |_g, dx, dy| {
            let _ = tx.send(AppAction::Screenshot(ScreenshotAction::DragUpdate(dx as i32, dy as i32)));
        }
    ));

    drag.connect_drag_end(clone!(#[strong] tx, move |_, _, _| {
        let _ = tx.send(AppAction::Screenshot(ScreenshotAction::DragEnd));
        }
    ));

    widgets.drawing_area.add_controller(drag);

    let controller = EventControllerMotion::new();        
    controller.connect_motion(clone!(#[strong] tx, move |_c, x, y| {
            let _ = tx.send(AppAction::Screenshot(ScreenshotAction::MouseMove(x as i32, y as i32)));
        }
    ));        

    widgets.drawing_area.add_controller(controller);

    let controller = ShortcutController::new();
        
    controller.add_shortcut(Shortcut::new(
        Some(ShortcutTrigger::parse_string("Escape").unwrap()),
        Some(CallbackAction::new(clone!(
            #[strong] tx,
            move |_, _,| {
                let _ = tx.send(AppAction::Global(GlobalAction::Quit));
                glib::Propagation::Stop
            }
        )
    ))));
        
    // Ctrl+S
    controller.add_shortcut(Shortcut::new(
        Some(ShortcutTrigger::parse_string("<Primary>s").unwrap()),
        Some(CallbackAction::new(clone!(
            #[strong] tx,
            move |_, _,| {
                let _ = tx.send(AppAction::Screenshot(ScreenshotAction::Save));
                glib::Propagation::Stop
            }
        )
    ))));

    // Ctrl+Z
    controller.add_shortcut(Shortcut::new(
        Some(ShortcutTrigger::parse_string("<Primary>z").unwrap()),
        Some(CallbackAction::new(clone!(
            #[strong] tx,
            move |_, _| {
                let _ = tx.send(AppAction::Screenshot(ScreenshotAction::Undo));
                glib::Propagation::Proceed
            }
        )))
    ));

    widgets.window.add_controller(controller);

    let key_controller = EventControllerKey::new();
    key_controller.connect_key_pressed(clone!(#[strong] tx, move |_, key, _, _| {
        if key == Key::Control_L || key == Key::Control_R {
            let _ = tx.send(AppAction::Screenshot(ScreenshotAction::ToogleMode));

            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
        }
    ));

    widgets.window.add_controller(key_controller);

}
