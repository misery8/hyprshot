use gdk4::Cursor;
use gtk4::prelude::WidgetExt;
use gtk4::DrawingArea;

use crate::modules::screenshot::state::{Rect, SelectionHitZone};

pub fn update_cursor(rect: &Rect, mouse_pos: (i32, i32), drawing_area: &DrawingArea) {
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

pub fn get_cursor_zone(
    rect: &Rect,
    mouse_pos: (i32, i32),
    margin: Option<i32>,
) -> SelectionHitZone {
    if rect.is_empty() {
        return SelectionHitZone::Outside;
    }

    let margin = margin.unwrap_or(10);

    let (x, y) = mouse_pos;

    let left = rect.x;
    let right = rect.x + rect.w;
    let top = rect.y;
    let bottom = rect.y + rect.h;

    let mut near_l = (x - left).abs() <= margin;
    let mut near_r = (x - right).abs() <= margin;
    let mut near_t = (y - top).abs() <= margin;
    let mut near_b = (y - bottom).abs() <= margin;

    if near_l && near_r {
        if (x - left).abs() <= (x - right).abs() {
            near_r = false;
        } else {
            near_l = false;
        }
    }

    if near_t && near_b {
        if (y - top).abs() <= (y - bottom).abs() {
            near_b = false;
        } else {
            near_t = false;
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_zone_tracks_effective_edge_after_axis_inversion() {
        let cases = [
            (
                Rect {
                    x: 120,
                    y: 30,
                    w: 30,
                    h: 80,
                },
                (150, 70),
                SelectionHitZone::E,
            ),
            (
                Rect {
                    x: 10,
                    y: 30,
                    w: 10,
                    h: 80,
                },
                (10, 70),
                SelectionHitZone::W,
            ),
            (
                Rect {
                    x: 20,
                    y: 110,
                    w: 100,
                    h: 20,
                },
                (70, 130),
                SelectionHitZone::S,
            ),
            (
                Rect {
                    x: 20,
                    y: 10,
                    w: 100,
                    h: 20,
                },
                (70, 10),
                SelectionHitZone::N,
            ),
        ];

        for (rect, pointer, expected) in cases {
            assert_eq!(get_cursor_zone(&rect, pointer, Some(10)), expected);
        }
    }

    #[test]
    fn cursor_zone_tracks_effective_corner_after_inversion() {
        let cases = [
            (
                Rect {
                    x: 120,
                    y: 110,
                    w: 30,
                    h: 20,
                },
                (150, 130),
                SelectionHitZone::SE,
            ),
            (
                Rect {
                    x: 10,
                    y: 110,
                    w: 10,
                    h: 20,
                },
                (10, 130),
                SelectionHitZone::SW,
            ),
            (
                Rect {
                    x: 120,
                    y: 10,
                    w: 30,
                    h: 20,
                },
                (150, 10),
                SelectionHitZone::NE,
            ),
            (
                Rect {
                    x: 10,
                    y: 10,
                    w: 10,
                    h: 20,
                },
                (10, 10),
                SelectionHitZone::NW,
            ),
        ];

        for (rect, pointer, expected) in cases {
            assert_eq!(get_cursor_zone(&rect, pointer, Some(10)), expected);
        }
    }

    #[test]
    fn narrow_crossing_rect_uses_nearest_opposite_edge() {
        let horizontal = Rect {
            x: 119,
            y: 30,
            w: 1,
            h: 80,
        };
        assert_eq!(
            get_cursor_zone(&horizontal, (120, 70), Some(10)),
            SelectionHitZone::E
        );
        assert_eq!(
            get_cursor_zone(&horizontal, (119, 70), Some(10)),
            SelectionHitZone::W
        );

        let vertical = Rect {
            x: 20,
            y: 109,
            w: 100,
            h: 1,
        };
        assert_eq!(
            get_cursor_zone(&vertical, (70, 110), Some(10)),
            SelectionHitZone::S
        );
        assert_eq!(
            get_cursor_zone(&vertical, (70, 109), Some(10)),
            SelectionHitZone::N
        );
    }
}
