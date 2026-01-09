use gdk::Cursor;
use gtk::DrawingArea;
use gtk::prelude::WidgetExt;

use crate::modules::screenshot::state::{Rect, SelectionHitZone};

pub fn update_cursor(
    rect: &Rect,
    mouse_pos: (i32, i32),
    drawing_area: &DrawingArea
) {
    let zone = get_cursor_zone(rect, mouse_pos, Some(10));
    apply_cursor(drawing_area, &zone);
}

pub fn apply_cursor(drawing_area: &DrawingArea, zone: &SelectionHitZone) {
    let cursor_name = match zone {
        SelectionHitZone::Inside => "move",
        SelectionHitZone::Outside => "default",
        SelectionHitZone::N | SelectionHitZone::S => "ns-resize",
        SelectionHitZone::E | SelectionHitZone::W => "ew-resize",
        SelectionHitZone::NW | SelectionHitZone::SE => "nwse-resize",
        SelectionHitZone::NE | SelectionHitZone::SW => "nesw-resize",
    };

    if let Some(cursor) = Cursor::from_name(cursor_name, None) {
        drawing_area.set_cursor(Some(&cursor));
    }
}

pub fn get_cursor_zone(rect: &Rect, mause_pos: (i32, i32), margin: Option<i32>) -> SelectionHitZone {
    
    if rect.is_empty() {
        return SelectionHitZone::Outside;
    }

    let margin = margin.unwrap_or(10);

    let (x, y) = mause_pos;

    let left = rect.x;
    let right = rect.x + rect.w;
    let top = rect.y;
    let bottom = rect.y + rect.h;

    let near_l = (x - left).abs() <= margin;
    let near_r = (x - right).abs() <= margin;
    let near_t = (y - top).abs() <= margin;
    let near_b = (y - bottom).abs() <= margin;

    let inside_x = x > left && x < right;
    let inside_y = y > top && y < bottom;

    match (near_l, near_r, near_t, near_b) {
        (true, _, true, _) => SelectionHitZone::NW,
        (_, true, true, _) => SelectionHitZone::NE,
        (true, _, _, true) => SelectionHitZone::SW,
        (_, true, _, true) => SelectionHitZone::SE,

        (_, _, true, _) if inside_x => SelectionHitZone::N,
        (_, _, _, true) if inside_x => SelectionHitZone::S,
        (true, _, _, _) if inside_y => SelectionHitZone::W,
        (_, true, _, _) if inside_y => SelectionHitZone::E,

        _ if inside_x && inside_y => SelectionHitZone::Inside,
        _ => SelectionHitZone::Outside,
    }
}
