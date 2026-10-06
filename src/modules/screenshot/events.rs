use std::sync::mpsc::Sender;

use gdk4::{Key, ModifierType};
use glib::clone;
use gtk4::{
    EventControllerMotion, GestureDrag, EventControllerKey,
    Shortcut, CallbackAction, ShortcutController, ShortcutTrigger, prelude::*
};

use crate::action::{AppAction, ScreenshotAction};
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
    key_controller.connect_key_pressed(clone!(#[strong] tx, move |_, key, _, modifiers| {
        if key == Key::Control_L || key == Key::Control_R {
            let _ = tx.send(AppAction::Screenshot(ScreenshotAction::ToggleMode));
            return glib::Propagation::Stop;
        }

        if key == Key::Escape {
            let _ = tx.send(AppAction::Screenshot(ScreenshotAction::Escape));
            return glib::Propagation::Stop;
        }

        if key == Key::BackSpace {
            let _ = tx.send(AppAction::Screenshot(ScreenshotAction::TextBackspace));
            return glib::Propagation::Stop;
        }

        if key == Key::Return || key == Key::KP_Enter {
            let _ = tx.send(AppAction::Screenshot(ScreenshotAction::TextCommit));
            return glib::Propagation::Stop;
        }

        let command_modifiers = ModifierType::CONTROL_MASK
            | ModifierType::ALT_MASK
            | ModifierType::SUPER_MASK
            | ModifierType::META_MASK
            | ModifierType::HYPER_MASK;

        if modifiers.intersects(command_modifiers) {
            return glib::Propagation::Proceed;
        }

        if let Some(ch) = key.to_unicode() {
            if !ch.is_control() {
                let _ = tx.send(AppAction::Screenshot(ScreenshotAction::TextInput(ch)));
                return glib::Propagation::Stop;
            }
        }

        glib::Propagation::Proceed
    }));

    widgets.window.add_controller(key_controller);
}
