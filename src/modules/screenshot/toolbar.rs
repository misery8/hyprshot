use std::cell::Cell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use glib::clone;
use gtk::{Box, Button, CssProvider, DrawingArea, Grid, Image, Overlay, Popover, ToggleButton};
use gtk::{prelude::*};

use crate::action::{AppAction, ScreenshotAction};
use crate::modules::screenshot::state::{Rect, Tool};

macro_rules! create_exlusive_toolbuttons {
    (
        tx = $tx:expr,
        container = $container:expr,
        active_by_default = $default_tool:expr,
        tools = [$( ($icon_path:expr, $tool_variant:path) ),* $(,)?]
    ) => {{
        let mut tool_buttons = Vec::new();

        $(
            
            let icon = Image::from_resource($icon_path);
            icon.set_opacity(1.0);
            icon.set_pixel_size(24);

            let button = ToggleButton::builder()
                .child(&icon)
                .focusable(false)
                .can_focus(false)
                .width_request(36).height_request(36)
                .build();
                        
            tool_buttons.push((button.clone(), $tool_variant));
        )*

        let tx = $tx.clone();
        let tool_buttons = Rc::new(tool_buttons);

        for (button, variant) in tool_buttons.iter() {
            let current_variant = *variant;
                        
            button.connect_clicked(clone!(
                #[strong] tool_buttons,
                #[strong] tx,
                #[strong] button,
                move |_| {
                    if button.is_active() {
                        for (other_button, _) in tool_buttons.iter() {
                            if !other_button.eq(&button) && other_button.is_active() {
                                other_button.set_active(false);
                            }
                        }
                        let _ = tx.send(AppAction::Screenshot(ScreenshotAction::SetTool(current_variant)));
                    } else {
                        let _ = tx.send(AppAction::Screenshot(ScreenshotAction::SetTool(Tool::None)));
                    }
                }
            ));

            $container.append(button);
        }

        tool_buttons

    }};
}

#[derive(Debug, Clone)]
pub struct Toolbar {
    container: Box,
}

impl Toolbar {
    pub fn new(tx: Sender<AppAction>) -> Self {
        let container = Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6).focusable(false)
            .halign(gtk::Align::Start).valign(gtk::Align::Start)
            .css_name("toolbar")
            .can_target(true)
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
        let _tool_buttons = create_exlusive_toolbuttons! {
            tx = tx,
            container = self.container,
            active_by_default = Tool::None,
            tools = [
                ("/io/github/misery8/hyprshot/icons/symbolic/diagonal-arrow-symbolic.svg", Tool::Arrow),
                ("/io/github/misery8/hyprshot/icons/symbolic/rectangle-symbolic.svg", Tool::Rectangle),
                ("/io/github/misery8/hyprshot/icons/symbolic/drop-water-symbolic.svg", Tool::Blur),
            ]
        };
    }

    fn setup_undo_button(&self, _tx: Sender<AppAction>) {
        let button = default_button("/io/github/misery8/hyprshot/icons/symbolic/undo-symbolic.svg");
        // button.connect_clicked(clone!(#[strong] tx, move |_| tx.send(AppAction::Screenshot(ScreenshotAction::Undo))));
        self.container.append(&button);
    }

    fn setup_color_picker_button(&self, tx: Sender<AppAction>) {
        let current_color = Rc::new(Cell::new((255u8, 0u8, 0u8)));

        let color_indicator = DrawingArea::builder()
            .width_request(12).height_request(12)
            .halign(gtk::Align::End).valign(gtk::Align::End)
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
                .add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

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
            }));

            grid.attach(&color_button, (index % 4) as i32, (index / 4) as i32, 1, 1);

        }

        grid
    }

    pub fn widget(&self) -> &Box { &self.container }

    pub fn update_position(&self, rect: &Rect) {
        let allocation = self.container.allocation();
        let x_pos = (rect.x + rect.w - allocation.width()).max(10);
        
        self.container.set_margin_start(x_pos);
        self.container.set_margin_top(rect.y + rect.h + 8);
    }

}

fn default_button(icon: &str) -> Button {
    
    let icon = Image::from_resource(icon);
    icon.set_opacity(0.6);
    icon.set_pixel_size(20);

    let button = Button::builder()
        .width_request(36).height_request(36)
        .focusable(false)
        .child(&icon)
        .build();
        
    button
}
