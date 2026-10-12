use std::cell::Cell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box, Button, CssProvider, Grid, Image, MenuButton, Overlay, Popover};

use crate::action::{AppAction, ScreenshotAction};
use crate::modules::screenshot::state::{Rect, Tool};

const TOOLBAR_GAP: i32 = 8;
const BUTTON_SIZE: i32 = 36;
const TOOL_ICON_SIZE: i32 = 24;
const PALETTE_ICON_SIZE: i32 = 28;
const COLOR_INDICATOR_SIZE: i32 = 12;
const COLOR_SWATCH_SIZE: i32 = 24;
const ACTIVE_TOOL_CLASS: &str = "suggested-action";
const COLOR_INDICATOR_CLASS: &str = "color-indicator";
const TOOLBAR_CLASS: &str = "hyprshot-toolbar";
const ICON_THEME_RESOURCE_PATH: &str = "/io/github/misery8/hyprshot/icons";

const ARROW_ICON: &str = "/io/github/misery8/hyprshot/icons/symbolic/diagonal-arrow-symbolic.svg";
const ARROW_ACTIVE_ICON: &str =
    "/io/github/misery8/hyprshot/icons/symbolic/diagonal-arrow-active-symbolic.svg";
const RECTANGLE_ICON: &str = "/io/github/misery8/hyprshot/icons/symbolic/rectangle-symbolic.svg";
const RECTANGLE_ACTIVE_ICON: &str =
    "/io/github/misery8/hyprshot/icons/symbolic/rectangle-active-symbolic.svg";
const TEXT_ICON: &str = "/io/github/misery8/hyprshot/icons/symbolic/text-symbolic.svg";
const TEXT_ACTIVE_ICON: &str =
    "/io/github/misery8/hyprshot/icons/symbolic/text-active-symbolic.svg";
const BLUR_ICON: &str = "/io/github/misery8/hyprshot/icons/symbolic/drop-water-symbolic.svg";
const BLUR_ACTIVE_ICON: &str =
    "/io/github/misery8/hyprshot/icons/symbolic/drop-water-active-symbolic.svg";
const UNDO_ICON: &str = "/io/github/misery8/hyprshot/icons/symbolic/undo-symbolic.svg";
const PALETTE_ICON: &str = "/io/github/misery8/hyprshot/icons/symbolic/palette-symbolic.svg";

fn next_tool(current: Tool, clicked: Tool) -> Tool {
    if current == clicked {
        Tool::None
    } else {
        clicked
    }
}

fn tool_icon_resources(tool: Tool) -> Option<(&'static str, &'static str)> {
    match tool {
        Tool::Arrow => Some((ARROW_ICON, ARROW_ACTIVE_ICON)),
        Tool::Rectangle => Some((RECTANGLE_ICON, RECTANGLE_ACTIVE_ICON)),
        Tool::Text => Some((TEXT_ICON, TEXT_ACTIVE_ICON)),
        Tool::Blur => Some((BLUR_ICON, BLUR_ACTIVE_ICON)),
        Tool::None => None,
    }
}

fn themed_icon_name(resource: &str) -> String {
    let filename = resource
        .rsplit('/')
        .next()
        .expect("toolbar icon resource must include a filename");
    let basename = filename
        .strip_suffix(".svg")
        .expect("toolbar icon resource must end in .svg");
    format!("hyprshot-{basename}")
}

fn icon_image_sized(resource: &str, size: i32) -> Image {
    // Only named GTK icons honor GtkImage::pixel-size and theme recoloring.
    let icon = Image::from_icon_name(&themed_icon_name(resource));
    icon.set_opacity(1.0);
    icon.set_pixel_size(size);
    icon
}

fn icon_image(resource: &str) -> Image {
    icon_image_sized(resource, TOOL_ICON_SIZE)
}

fn tool_button(tool: Tool) -> Button {
    let (icon_resource, _) =
        tool_icon_resources(tool).expect("annotation tool must have icon resources");
    let icon = icon_image(icon_resource);

    Button::builder()
        .child(&icon)
        .focusable(false)
        .can_focus(false)
        .width_request(BUTTON_SIZE)
        .height_request(BUTTON_SIZE)
        .build()
}

fn set_tool_button_icon(button: &Button, tool: Tool, active: bool) {
    if let Some((normal_icon, active_icon)) = tool_icon_resources(tool) {
        let icon = icon_image(if active { active_icon } else { normal_icon });
        button.set_child(Some(&icon));
    }
}

fn toolbar_geometry_css() -> &'static str {
    // Paint the shared surface and button faces independently of GTK defaults.
    // Legacy theme colors adapt in light/dark modes; literal fallbacks cover
    // themes that omit the named colors.
    ".hyprshot-toolbar { \
         border-radius: 10px; \
         padding: 4px; \
         background-image: none; \
         background-color: rgba(231, 235, 239, 0.97); \
         background-color: mix(@theme_bg_color, @theme_fg_color, 0.07); \
         border: 1px solid rgba(100, 105, 112, 0.22); \
         border-color: alpha(@theme_fg_color, 0.14); \
     } \
     .hyprshot-toolbar > button, \
     .hyprshot-toolbar > menubutton > button { \
         min-width: 26px; \
         min-height: 30px; \
         padding: 2px; \
         border-radius: 6px; \
         background-image: none; \
         background-color: rgba(244, 247, 250, 0.98); \
         background-color: mix(@theme_bg_color, @theme_fg_color, 0.02); \
         border: 1px solid rgba(100, 105, 112, 0.15); \
         border-color: alpha(@theme_fg_color, 0.14); \
     } \
     .hyprshot-toolbar > button.suggested-action { \
         background-color: #3584e4; \
         background-color: @theme_selected_bg_color; \
         color: #ffffff; \
         color: @theme_selected_fg_color; \
     }"
}

fn color_indicator_css((red, green, blue): (u8, u8, u8)) -> String {
    format!(
        ".{COLOR_INDICATOR_CLASS} {{ \
            background-color: rgb({red}, {green}, {blue}); \
            border-radius: 0px; \
            border: 1px solid rgba(255, 255, 255, 0.85); \
        }}"
    )
}

#[derive(Debug, Clone)]
pub struct Toolbar {
    container: Box,
}

impl Toolbar {
    pub fn new(tx: Sender<AppAction>) -> Self {
        let container = Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .focusable(false)
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::Start)
            .css_name("toolbar")
            .can_target(false)
            .opacity(0.0)
            .hexpand(false)
            .vexpand(false)
            .build();

        // Register the compiled symbolic files in GTK's hicolor resource namespace.
        // The aliases in resources.gresource.xml preserve the original resource URLs.
        gtk4::IconTheme::for_display(&container.display())
            .add_resource_path(ICON_THEME_RESOURCE_PATH);

        let toolbar_provider = CssProvider::new();
        toolbar_provider.load_from_data(toolbar_geometry_css());
        container.add_css_class(TOOLBAR_CLASS);
        gtk4::style_context_add_provider_for_display(
            &container.display(),
            &toolbar_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let toolbar = Self { container };

        toolbar.setup_drawing_tools(tx.clone());
        toolbar.setup_undo_button(tx.clone());
        toolbar.setup_color_picker_button(tx.clone());

        toolbar
    }

    fn setup_drawing_tools(&self, tx: Sender<AppAction>) {
        let tool_buttons = vec![
            (tool_button(Tool::Arrow), Tool::Arrow),
            (tool_button(Tool::Rectangle), Tool::Rectangle),
            (tool_button(Tool::Text), Tool::Text),
            (tool_button(Tool::Blur), Tool::Blur),
        ];

        let active_tool = Rc::new(Cell::new(Tool::None));
        let weak_buttons: Vec<_> = tool_buttons
            .iter()
            .map(|(button, tool)| (button.downgrade(), *tool))
            .collect();

        for (button, variant) in &tool_buttons {
            let current_variant = *variant;
            let active_tool = active_tool.clone();
            let buttons = weak_buttons.clone();
            let tx = tx.clone();

            button.connect_clicked(move |_| {
                let tool = next_tool(active_tool.get(), current_variant);
                active_tool.set(tool);

                for (button, button_tool) in &buttons {
                    if let Some(button) = button.upgrade() {
                        let is_active = *button_tool == tool;

                        if is_active {
                            button.add_css_class(ACTIVE_TOOL_CLASS);
                        } else {
                            button.remove_css_class(ACTIVE_TOOL_CLASS);
                        }

                        set_tool_button_icon(&button, *button_tool, is_active);
                    }
                }

                let _ = tx.send(AppAction::Screenshot(ScreenshotAction::SetTool(tool)));
            });
        }

        for (button, _) in &tool_buttons {
            self.container.append(button);
        }
    }

    fn setup_undo_button(&self, tx: Sender<AppAction>) {
        let button = default_button(UNDO_ICON);
        button.connect_clicked(clone!(
            #[strong]
            tx,
            move |_| {
                let _ = tx.send(AppAction::Screenshot(ScreenshotAction::Undo));
            }
        ));
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
            .margin_end(1)
            .margin_bottom(0)
            .can_target(false)
            .build();
        color_indicator.add_css_class(COLOR_INDICATOR_CLASS);
        color_indicator.style_context().add_provider(
            &indicator_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let icon = icon_image_sized(PALETTE_ICON, PALETTE_ICON_SIZE);

        let overlay = Overlay::builder().child(&icon).build();
        overlay.add_overlay(&color_indicator);

        let popover = Popover::builder().autohide(true).build();

        let grid = Self::build_color_picker_grid(tx, current_color, indicator_provider, &popover);
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
            (255, 0, 0),
            (0, 255, 0),
            (0, 0, 255),
            (255, 255, 0),
            (255, 0, 255),
            (0, 255, 255),
            (255, 128, 0),
            (128, 255, 0),
            (0, 128, 255),
            (128, 0, 255),
            (255, 0, 128),
            (0, 255, 128),
            (192, 192, 0),
            (128, 128, 128),
            (64, 64, 64),
            (0, 0, 0),
            (255, 255, 255),
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
            color_button
                .style_context()
                .add_provider(&provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);

            color_button.connect_clicked(clone!(
                #[strong]
                tx,
                #[strong]
                indicator_color,
                #[strong]
                indicator_provider,
                #[weak]
                popover,
                move |_| {
                    let _ = tx.send(AppAction::Screenshot(ScreenshotAction::SetColor(
                        red, green, blue,
                    )));

                    indicator_color.set((red, green, blue));
                    indicator_provider.load_from_data(&color_indicator_css((red, green, blue)));
                    popover.popdown();
                }
            ));

            grid.attach(&color_button, (index % 4) as i32, (index / 4) as i32, 1, 1);
        }

        grid
    }

    pub fn widget(&self) -> &Box {
        &self.container
    }

    pub fn update_position(&self, rect: &Rect, screen_size: (i32, i32)) {
        let allocation = self.container.allocation();
        let position =
            calculate_position(rect, screen_size, (allocation.width(), allocation.height()));

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
        if top_y >= TOOLBAR_GAP {
            top_y
        } else {
            TOOLBAR_GAP
        },
    )
}

fn default_button(icon_resource: &str) -> Button {
    let icon = icon_image(icon_resource);

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
    fn active_tool_icons_have_dedicated_resources() {
        for tool in [Tool::Arrow, Tool::Rectangle, Tool::Text, Tool::Blur] {
            let (normal_icon, active_icon) = tool_icon_resources(tool).unwrap();

            assert_ne!(normal_icon, active_icon);
            assert!(active_icon.contains("active"));
        }
    }

    #[test]
    fn gtk_theme_resolves_toolbar_symbols_at_requested_size() {
        if std::env::var_os("HYPRSHOT_GTK_RENDER_TEST").is_none() {
            return;
        }

        gtk4::init().expect("GTK4 display required for symbolic icon integration");
        gtk4::gio::resources_register_include!("compiled.gresource")
            .expect("symbolic GResource must register");

        let (tx, _rx) = std::sync::mpsc::channel();
        let toolbar = Toolbar::new(tx);
        let theme = gtk4::IconTheme::for_display(&toolbar.widget().display());

        for resource in [
            ARROW_ICON,
            ARROW_ACTIVE_ICON,
            RECTANGLE_ICON,
            RECTANGLE_ACTIVE_ICON,
            TEXT_ICON,
            TEXT_ACTIVE_ICON,
            BLUR_ICON,
            BLUR_ACTIVE_ICON,
            UNDO_ICON,
            PALETTE_ICON,
        ] {
            let icon_name = themed_icon_name(resource);
            assert!(theme.has_icon(&icon_name), "GTK cannot find {icon_name}");
        }

        let image = icon_image(ARROW_ICON);
        assert_eq!(image.storage_type(), gtk4::ImageType::IconName);
        assert_eq!(image.pixel_size(), TOOL_ICON_SIZE);
        assert_eq!(
            image.icon_name().as_deref(),
            Some("hyprshot-diagonal-arrow-symbolic")
        );

        let palette = icon_image_sized(PALETTE_ICON, PALETTE_ICON_SIZE);
        assert_eq!(palette.storage_type(), gtk4::ImageType::IconName);
        assert_eq!(palette.pixel_size(), PALETTE_ICON_SIZE);

        let window = gtk4::Window::new();
        toolbar.widget().set_opacity(1.0);
        window.set_child(Some(toolbar.widget()));
        window.present();
        while gtk4::glib::MainContext::default().pending() {
            gtk4::glib::MainContext::default().iteration(false);
        }

        let (_, natural_width, _, _) = toolbar.widget().measure(gtk4::Orientation::Horizontal, -1);
        let (_, natural_height, _, _) = toolbar.widget().measure(gtk4::Orientation::Vertical, -1);
        eprintln!("GTK toolbar natural size: {natural_width}x{natural_height}");
        let mut child = toolbar.widget().first_child();
        let mut button_count = 0;
        let mut first_width = None;
        while let Some(widget) = child {
            let (minimum, natural, _, _) = widget.measure(gtk4::Orientation::Horizontal, -1);
            eprintln!(
                "GTK toolbar child {} width_request={} minimum={} natural={}",
                widget.css_name(),
                widget.width_request(),
                minimum,
                natural
            );
            assert!(natural >= BUTTON_SIZE, "release button was shrunk");
            if let Some(first_width) = first_width {
                assert_eq!(
                    natural, first_width,
                    "palette and tools must have equal button widths"
                );
            } else {
                first_width = Some(natural);
            }
            button_count += 1;
            child = widget.next_sibling();
        }
        assert_eq!(button_count, 6, "expected five tools and a palette button");
        assert!(
            (250..=285).contains(&natural_width) && (42..=55).contains(&natural_height),
            "GTK toolbar does not preserve release-size controls"
        );

        window.close();
    }

    #[test]
    fn toolbar_named_symbolic_icons_have_the_expected_resource_identity() {
        for resource in [
            ARROW_ICON,
            ARROW_ACTIVE_ICON,
            RECTANGLE_ICON,
            RECTANGLE_ACTIVE_ICON,
            TEXT_ICON,
            TEXT_ACTIVE_ICON,
            BLUR_ICON,
            BLUR_ACTIVE_ICON,
            UNDO_ICON,
            PALETTE_ICON,
        ] {
            let name = themed_icon_name(resource);
            assert!(name.starts_with("hyprshot-"));
            assert!(name.ends_with("-symbolic"));
        }

        assert_eq!(TOOL_ICON_SIZE, 24);
        assert_eq!(PALETTE_ICON_SIZE, 28);
    }

    #[test]
    fn toolbar_geometry_css_restores_compact_theme_aware_button_group() {
        let css = toolbar_geometry_css();

        assert_eq!(BUTTON_SIZE, 36);
        assert_eq!(COLOR_INDICATOR_SIZE, 12);
        assert!(css.contains("border-radius: 10px"));
        assert!(css.contains("padding: 4px"));
        assert!(css.contains("background-image: none"));
        assert!(css.contains("background-color: rgba(231, 235, 239, 0.97)"));
        assert!(css.contains("mix(@theme_bg_color, @theme_fg_color, 0.07)"));
        assert!(css.contains("background-color: rgba(244, 247, 250, 0.98)"));
        assert!(css.contains(".hyprshot-toolbar > menubutton > button"));
        assert!(css.contains(".hyprshot-toolbar > button.suggested-action"));
        assert!(css.contains("min-width: 26px"));
        assert!(css.contains("min-height: 30px"));
    }

    #[test]
    fn color_indicator_css_tracks_selected_color() {
        let css = color_indicator_css((12, 34, 56));

        assert!(css.contains("rgb(12, 34, 56)"));
        assert!(css.contains("color-indicator"));
        assert!(css.contains("border-radius: 0px"));
    }

    #[test]
    fn toolbar_uses_selection_bottom_right_when_it_fits() {
        let rect = Rect {
            x: 50,
            y: 40,
            w: 100,
            h: 80,
        };

        assert_eq!(calculate_position(&rect, (300, 300), (80, 36)), (70, 128),);
    }

    #[test]
    fn toolbar_falls_back_to_selection_top_center() {
        let rect = Rect {
            x: 50,
            y: 200,
            w: 100,
            h: 80,
        };

        assert_eq!(calculate_position(&rect, (300, 300), (80, 36)), (60, 156),);
    }

    #[test]
    fn toolbar_uses_top_margin_when_selection_has_no_room_above() {
        let rect = Rect {
            x: 0,
            y: 0,
            w: 300,
            h: 300,
        };

        assert_eq!(calculate_position(&rect, (300, 300), (80, 36)), (110, 8),);
    }

    #[test]
    fn toolbar_position_is_safe_when_toolbar_is_wider_than_screen() {
        let rect = Rect {
            x: 0,
            y: 50,
            w: 100,
            h: 50,
        };

        assert_eq!(calculate_position(&rect, (100, 200), (120, 36)), (0, 108),);
    }
}
