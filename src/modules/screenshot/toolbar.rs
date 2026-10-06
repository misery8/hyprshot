use std::cell::Cell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use glib::clone;
use gtk4::{Box, Button, CssProvider, DrawingArea, Grid, Image, Overlay, Popover};
use gtk4::prelude::*;

use crate::action::{AppAction, ScreenshotAction};
use crate::modules::screenshot::state::{Rect, Tool};

const TOOLBAR_GAP: i32 = 8;
const ACTIVE_TOOL_CLASS: &str = "suggested-action";

fn next_tool(current: Tool, clicked: Tool) -> Tool {
    if current == clicked {
        Tool::None
    } else {
        clicked
    }
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
            icon.set_pixel_size(24);

            let button = Button::builder()
                .child(&icon)
                .focusable(false)
                .can_focus(false)
                .width_request(36)
                .height_request(36)
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
        create_exclusive_toolbuttons! {
            tx = tx,
            container = self.container,
            tools = [
                ("/io/github/misery8/hyprshot/icons/symbolic/diagonal-arrow-symbolic.svg", Tool::Arrow),
                ("/io/github/misery8/hyprshot/icons/symbolic/rectangle-symbolic.svg", Tool::Rectangle),
                ("/io/github/misery8/hyprshot/icons/symbolic/drop-water-symbolic.svg", Tool::Blur),
            ]
        };
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

        let color_indicator = DrawingArea::builder()
            .width_request(12).height_request(12)
            .halign(gtk4::Align::End).valign(gtk4::Align::End)
            .margin_end(2).margin_bottom(2)
            .build();

        color_indicator.set_draw_func(clone!(#[strong] current_color,
            move |_, cr, w, h| {
                let (r, g, b) = current_color.get();
                cr.set_source_rgb(r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
                cr.rectangle(0.0, 0.0, w as f64, h as f64);
                let _ = cr.fill();
            }
        ));

        let icon = Image::from_resource("/io/github/misery8/hyprshot/icons/symbolic/palette-symbolic.svg");
        icon.set_size_request(24, 24);

        let overlay = Overlay::builder()
            .child(&icon)
            .build();
        overlay.add_overlay(&color_indicator);

        let button = Button::builder()
            .width_request(36).height_request(36)
            .focusable(false)
            .child(&overlay)
            .build();

        let popover = Popover::builder()
            .autohide(true)
            .build();
        popover.set_parent(&button);

        let grid = Self::build_color_picker_grid(
            tx,
            current_color,
            &color_indicator,
            &popover
        );
        popover.set_child(Some(&grid));

        button.connect_clicked(clone!(#[strong] popover, move |_| popover.popup()));

        self.container.append(&button);
    }

    fn build_color_picker_grid(
        tx: Sender<AppAction>,
        indicator_color: Rc<Cell<(u8, u8, u8)>>,
        drawing_area: &DrawingArea,
        popover: &Popover,
    ) -> Grid {
        let grid = Grid::builder()
            .row_spacing(2)
            .column_spacing(2)
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
                .width_request(20).height_request(20)
                .build();

            let color_css = format!(
                "button {{ background: rgb({red}, {green}, {blue}); border: 1px solid #ccc; }}"
            );

            let provider = CssProvider::new();
            provider.load_from_data(&color_css);
            color_button.style_context()
                .add_provider(&provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);

            color_button.connect_clicked(clone!(
                #[strong] tx,
                #[strong] indicator_color,
                #[strong] drawing_area,
                #[weak] popover,
                move |_| {
                    let _ = tx.send(AppAction::Screenshot(ScreenshotAction::SetColor(red, green, blue)));

                    indicator_color.set((red, green, blue));
                    drawing_area.queue_draw();
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
    icon.set_pixel_size(20);

    Button::builder()
        .width_request(36).height_request(36)
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
