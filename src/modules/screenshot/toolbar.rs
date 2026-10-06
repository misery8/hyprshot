use std::cell::Cell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use glib::clone;
use gtk4::{Box, Button, CssProvider, Grid, Image, MenuButton, Overlay, Popover};
use gtk4::prelude::*;

use crate::action::{AppAction, ScreenshotAction};
use crate::modules::screenshot::state::{Rect, Tool};

const TOOLBAR_GAP: i32 = 8;
const BUTTON_SIZE: i32 = 36;
const TOOL_ICON_SIZE: i32 = 24;
const ACTION_ICON_SIZE: i32 = 20;
const COLOR_INDICATOR_SIZE: i32 = 12;
const COLOR_SWATCH_SIZE: i32 = 24;
const ACTIVE_TOOL_CLASS: &str = "suggested-action";
const COLOR_INDICATOR_CLASS: &str = "color-indicator";

fn next_tool(current: Tool, clicked: Tool) -> Tool {
    if current == clicked {
        Tool::None
    } else {
        clicked
    }
}

fn color_indicator_css((red, green, blue): (u8, u8, u8)) -> String {
    format!(
        ".{COLOR_INDICATOR_CLASS} {{ \
            background-color: rgb({red}, {green}, {blue}); \
            border-radius: 999px; \
            border: 1px solid rgba(255, 255, 255, 0.85); \
        }}"
    )
}

macro_rules! create_exclusive_toolbuttons {
    (
        tx = $tx:expr,
        container = $container:expr,
        tools = [$( ($icon_path:expr, $tool_variant:path) ),* $(,)?]
    ) => {{
        let mut tool_buttons = Vec::new();

        $(
            let icon = Image::from_resource($icon_path);
            icon.set_opacity(1.0);
            icon.set_pixel_size(TOOL_ICON_SIZE);

            let button = Button::builder()
                .child(&icon)
                .focusable(false)
                .can_focus(false)
                .width_request(BUTTON_SIZE)
                .height_request(BUTTON_SIZE)
                .build();

            tool_buttons.push((button, $tool_variant));
        )*

        let active_tool = Rc::new(Cell::new(Tool::None));
        let weak_buttons: Vec<_> = tool_buttons
            .iter()
            .map(|(button, tool)| (button.downgrade(), *tool))
            .collect();

        for (button, variant) in &tool_buttons {
            let current_variant = *variant;
            let active_tool = active_tool.clone();
            let buttons = weak_buttons.clone();
            let tx = $tx.clone();

            button.connect_clicked(move |_| {
                let tool = next_tool(active_tool.get(), current_variant);
                active_tool.set(tool);

                for (button, button_tool) in &buttons {
                    if let Some(button) = button.upgrade() {
                        if *button_tool == tool {
                            button.add_css_class(ACTIVE_TOOL_CLASS);
                        } else {
                            button.remove_css_class(ACTIVE_TOOL_CLASS);
                        }
                    }
                }

                let _ = tx.send(AppAction::Screenshot(ScreenshotAction::SetTool(tool)));
            });

            $container.append(button);
        }
    }};
}

#[derive(Debug, Clone)]
pub struct Toolbar {
    container: Box,
}

impl Toolbar {
    pub fn new(tx: Sender<AppAction>) -> Self {
        let container = Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6).focusable(false)
            .halign(gtk4::Align::Start).valign(gtk4::Align::Start)
            .css_name("toolbar")
            .can_target(false)
            .opacity(0.0)
            .hexpand(false)
            .vexpand(false)
            .build();

        let toolbar = Self { container };

        toolbar.setup_drawing_tools(tx.clone());
        toolbar.setup_undo_button(tx.clone());
        toolbar.setup_color_picker_button(tx.clone());

        toolbar
    }

    fn setup_drawing_tools(&self, tx: Sender<AppAction>) {
        let tool_group = Box::new(gtk4::Orientation::Horizontal, 0);
        tool_group.add_css_class("linked");
        tool_group.set_focusable(false);

        create_exclusive_toolbuttons! {
            tx = tx,
            container = tool_group,
            tools = [
                ("/io/github/misery8/hyprshot/icons/symbolic/diagonal-arrow-symbolic.svg", Tool::Arrow),
                ("/io/github/misery8/hyprshot/icons/symbolic/rectangle-symbolic.svg", Tool::Rectangle),
                ("/io/github/misery8/hyprshot/icons/symbolic/drop-water-symbolic.svg", Tool::Blur),
            ]
        };

        self.container.append(&tool_group);
    }

    fn setup_undo_button(&self, tx: Sender<AppAction>) {
        let button = default_button("/io/github/misery8/hyprshot/icons/symbolic/undo-symbolic.svg");
        button.connect_clicked(clone!(#[strong] tx, move |_| {
            let _ = tx.send(AppAction::Screenshot(ScreenshotAction::Undo));
        }));
        self.container.append(&button);
    }

    fn setup_color_picker_button(&self, tx: Sender<AppAction>) {
        let current_color = Rc::new(Cell::new((255u8, 0u8, 0u8)));

        let indicator_provider = CssProvider::new();
        indicator_provider.load_from_data(&color_indicator_css(current_color.get()));

        let color_indicator = Box::builder()
            .width_request(COLOR_INDICATOR_SIZE)
            .height_request(COLOR_INDICATOR_SIZE)
            .halign(gtk4::Align::End)
            .valign(gtk4::Align::End)
            .margin_end(2)
            .margin_bottom(2)
            .can_target(false)
            .build();
        color_indicator.add_css_class(COLOR_INDICATOR_CLASS);
        color_indicator.style_context()
            .add_provider(&indicator_provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);

        let icon = Image::from_resource("/io/github/misery8/hyprshot/icons/symbolic/palette-symbolic.svg");
        icon.set_pixel_size(TOOL_ICON_SIZE);

        let overlay = Overlay::builder()
            .child(&icon)
            .build();
        overlay.add_overlay(&color_indicator);

        let popover = Popover::builder()
            .autohide(true)
            .build();

        let grid = Self::build_color_picker_grid(
            tx,
            current_color,
            indicator_provider,
            &popover,
        );
        popover.set_child(Some(&grid));

        let button = MenuButton::builder()
            .width_request(BUTTON_SIZE)
            .height_request(BUTTON_SIZE)
            .focusable(false)
            .build();
        button.set_child(Some(&overlay));
        button.set_popover(Some(&popover));

        self.container.append(&button);
    }

    fn build_color_picker_grid(
        tx: Sender<AppAction>,
        indicator_color: Rc<Cell<(u8, u8, u8)>>,
        indicator_provider: CssProvider,
        popover: &Popover,
    ) -> Grid {
        let grid = Grid::builder()
            .row_spacing(2)
            .column_spacing(2)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();

        const COLOR_PALETTE: &[(u8, u8, u8)] = &[
            (255, 0, 0), (0, 255, 0), (0, 0, 255),
            (255, 255, 0), (255, 0, 255), (0, 255, 255),
            (255, 128, 0), (128, 255, 0), (0, 128, 255),
            (128, 0, 255), (255, 0, 128), (0, 255, 128),
            (192, 192, 0), (128, 128, 128), (64, 64, 64),
            (0, 0, 0), (255, 255, 255),
        ];

        for (index, &(red, green, blue)) in COLOR_PALETTE.iter().enumerate() {
            let color_button = Button::builder()
                .width_request(COLOR_SWATCH_SIZE)
                .height_request(COLOR_SWATCH_SIZE)
                .build();

            let color_css = format!(
                "button {{ background: rgb({red}, {green}, {blue}); border: 1px solid #ccc; border-radius: 3px; }}"
            );

            let provider = CssProvider::new();
            provider.load_from_data(&color_css);
            color_button.style_context()
                .add_provider(&provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);

            color_button.connect_clicked(clone!(
                #[strong] tx,
                #[strong] indicator_color,
                #[strong] indicator_provider,
                #[weak] popover,
                move |_| {
                    let _ = tx.send(AppAction::Screenshot(ScreenshotAction::SetColor(red, green, blue)));

                    indicator_color.set((red, green, blue));
                    indicator_provider.load_from_data(&color_indicator_css((red, green, blue)));
                    popover.popdown();
                }
            ));

            grid.attach(&color_button, (index % 4) as i32, (index / 4) as i32, 1, 1);
        }

        grid
    }

    pub fn widget(&self) -> &Box { &self.container }

    pub fn update_position(&self, rect: &Rect, screen_size: (i32, i32)) {
        let allocation = self.container.allocation();
        let position = calculate_position(
            rect,
            screen_size,
            (allocation.width(), allocation.height()),
        );

        self.container.set_margin_start(position.0);
        self.container.set_margin_top(position.1);
    }
}

fn calculate_position(
    rect: &Rect,
    screen_size: (i32, i32),
    toolbar_size: (i32, i32),
) -> (i32, i32) {
    let (screen_w, screen_h) = screen_size;
    let (toolbar_w, toolbar_h) = toolbar_size;
    let max_x = (screen_w - toolbar_w).max(0);

    let right_x = (rect.x + rect.w - toolbar_w).clamp(0, max_x);
    let center_x = (rect.x + (rect.w - toolbar_w) / 2).clamp(0, max_x);
    let bottom_y = rect.y + rect.h + TOOLBAR_GAP;

    if bottom_y + toolbar_h <= screen_h {
        return (right_x, bottom_y);
    }

    let top_y = rect.y - toolbar_h - TOOLBAR_GAP;
    (
        center_x,
        if top_y >= TOOLBAR_GAP { top_y } else { TOOLBAR_GAP },
    )
}

fn default_button(icon: &str) -> Button {
    let icon = Image::from_resource(icon);
    icon.set_opacity(0.6);
    icon.set_pixel_size(ACTION_ICON_SIZE);

    Button::builder()
        .width_request(BUTTON_SIZE)
        .height_request(BUTTON_SIZE)
        .focusable(false)
        .child(&icon)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_inactive_tool_selects_it() {
        assert_eq!(next_tool(Tool::None, Tool::Arrow), Tool::Arrow);
        assert_eq!(next_tool(Tool::Rectangle, Tool::Arrow), Tool::Arrow);
    }

    #[test]
    fn clicking_active_tool_returns_to_selection_mode() {
        assert_eq!(next_tool(Tool::Arrow, Tool::Arrow), Tool::None);
        assert_eq!(next_tool(Tool::Blur, Tool::Blur), Tool::None);
    }

    #[test]
    fn color_indicator_css_tracks_selected_color() {
        let css = color_indicator_css((12, 34, 56));

        assert!(css.contains("rgb(12, 34, 56)"));
        assert!(css.contains("color-indicator"));
    }

    #[test]
    fn toolbar_uses_selection_bottom_right_when_it_fits() {
        let rect = Rect { x: 50, y: 40, w: 100, h: 80 };

        assert_eq!(
            calculate_position(&rect, (300, 300), (80, 36)),
            (70, 128),
        );
    }

    #[test]
    fn toolbar_falls_back_to_selection_top_center() {
        let rect = Rect { x: 50, y: 200, w: 100, h: 80 };

        assert_eq!(
            calculate_position(&rect, (300, 300), (80, 36)),
            (60, 156),
        );
    }

    #[test]
    fn toolbar_uses_top_margin_when_selection_has_no_room_above() {
        let rect = Rect { x: 0, y: 0, w: 300, h: 300 };

        assert_eq!(
            calculate_position(&rect, (300, 300), (80, 36)),
            (110, 8),
        );
    }

    #[test]
    fn toolbar_position_is_safe_when_toolbar_is_wider_than_screen() {
        let rect = Rect { x: 0, y: 50, w: 100, h: 50 };

        assert_eq!(
            calculate_position(&rect, (100, 200), (120, 36)),
            (0, 108),
        );
    }
}
