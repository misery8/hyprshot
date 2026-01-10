use cairo::ImageSurface;

use crate::capture::screenshot::export::export_selection;
use crate::common::cursor;

#[derive(Debug, Clone)]
pub struct ScreenshotState {
    selection: Selection,
    paused: bool,
    mouse_pos: (i32, i32),
    current_tool: Tool,
    current_color: (u8, u8, u8),
    drag_start: Option<(i32, i32)>,
    drag_origin: Option<Rect>,
    drag_mode: Option<DragMode>,
    current_shape: Option<Shape>,
}

impl Default for ScreenshotState {
    
    fn default() -> Self {

        Self {
            selection: Selection::idle(),
            paused: false,
            mouse_pos: (0, 0),
            current_tool: Tool::None,
            current_color: (255, 0, 0),
            drag_start: None,
            drag_origin: None,
            drag_mode: None,
            current_shape: None
        }
    }
}

impl ScreenshotState {
    // Immutable
    pub fn selection(&self) -> &Selection { &self.selection }
    pub fn is_paused(&self) -> bool { self.paused }
    pub fn mouse_pos(&self) -> (i32, i32) { self.mouse_pos }
    pub fn current_shape(&self) -> Option<&Shape> { self.current_shape.as_ref() }
    pub fn current_tool(&self) -> Tool { self.current_tool }
    
    // Mutable
    pub fn toogle_pause(&mut self) {
        if self.selection.is_active() && !self.paused {
            self.paused = !self.paused;
        }
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.current_tool = tool;
    }

    pub fn set_color(&mut self, color: (u8, u8, u8)) {
        self.current_color = color;
    }

    pub fn begin_drag(&mut self, x: i32, y: i32) {
        self.mouse_pos = (x, y);

        if self.current_tool != Tool::None {
            if self.paused
                && self.selection.is_active()
                && self.selection.rect.contains((x, y))
            {
                self.current_shape = None;
                self.drag_start = Some((x, y));
            }
            return;
        }

        self.drag_start = Some((x, y));
        self.drag_origin = Some(self.selection.rect);

        let zone = cursor::get_cursor_zone(
            &self.selection.rect,
            self.mouse_pos,
            Some(10)
        );

        self.drag_mode = Some(match zone {
            SelectionHitZone::Outside => DragMode::Create,
            SelectionHitZone::Inside => DragMode::Move,
            z => DragMode::Resize(z),
        });

        self.selection = Selection::dragging(self.selection.rect);

    }

    pub fn update_drag(&mut self, dx: i32, dy: i32) {
        let Some((start_x, start_y)) = self.drag_start else { return; };

        if self.current_tool != Tool::None {
            self.current_shape = self.get_current_shape();
            return;
        }

        let Some(origin) = self.drag_origin else { return; };
        let Some(mode) = self.drag_mode else { return; };

        let cx = dx + start_x;
        let cy = dy + start_y;

        match mode {
            DragMode::Create => {
                self.selection.rect.x = start_x.min(cx);
                self.selection.rect.y = start_y.min(cy);
                self.selection.rect.w = (cx - start_x).abs();
                self.selection.rect.h = (cy - start_y).abs();
            }

            DragMode::Move => {
                if self.current_tool == Tool::None {
                    self.selection.rect.x = origin.x + dx;
                    self.selection.rect.y = origin.y + dy;
                }
            }

            DragMode::Resize(zone) => {
                self.selection.rect = self.resize_rect(&zone, dx, dy);
            }
        }

        self.mouse_pos = (start_x + dx, start_y + dy);
        
    }

    pub fn end_drag(&mut self) {
        self.drag_start = None;
        self.drag_origin = None;
        self.drag_mode = None;
        self.current_shape = None;

        if self.selection.is_active() {
            self.selection = Selection::finalized(self.selection.rect);
        } else {
            self.selection = Selection::idle();
        }
    }

    pub fn set_mouse_pos(&mut self, pos: (i32, i32)) {
        self.mouse_pos = pos;
    }

    pub fn export_selection(&self, original_surface: &ImageSurface) -> anyhow::Result<Vec<u8>> {
        export_selection(original_surface, self)
    }

    fn get_current_shape(&self) -> Option<Shape> {

        let from = self.drag_start.unwrap();
        let to = self.mouse_pos;

        match self.current_tool {
            Tool::Arrow => {
                return Some(Shape::Arrow { 
                    from: from,
                    to: to,
                    color: self.current_color
                });
            }

            Tool::Rectangle => {
                return Some(Shape::Rectangle {
                    rect: Self::rect_from_points(from, to),
                    color: self.current_color 
                });
            }

            Tool::Blur => {
                return Some(Shape::Blur { rect: Self::rect_from_points(from, to) });
            }
            _ => {None}
        }

    }

    fn resize_rect(&self, zone: &SelectionHitZone, x: i32, y: i32) -> Rect {

        let origin = self.drag_origin.unwrap();
        let mut rect = origin;

        match zone {
            // Corners
            SelectionHitZone::NW => {
                rect.x = origin.x + x;
                rect.y = origin.y + y;
                rect.w = (origin.w - x).max(1);
                rect.h = (origin.h - y).max(1);
            }

            SelectionHitZone::NE => {
                rect.y = origin.y + y;
                rect.w = (origin.w + x).max(1);
                rect.h = (origin.h - y).max(1);
            }

            SelectionHitZone::SE => {
                rect.w = (origin.w + x).max(1);
                rect.h = (origin.h + y).max(1);
            }

            SelectionHitZone::SW => {
                rect.x = origin.x + x;
                rect.w = (origin.w - x).max(1);
                rect.h = (origin.h + y).max(1);
            }
            
            // Sides
            SelectionHitZone::N => {
                rect.y = origin.y + y;
                rect.h = (origin.h - y).max(1);
            }

            SelectionHitZone::S => {
                rect.h = (origin.h + y).max(1);
            }

            SelectionHitZone::W => {
                rect.x = origin.x + x;
                rect.w = (origin.w - x).max(1);
            }

            SelectionHitZone::E => {
                rect.w = (origin.w + x).max(1);
            }

            _ => {}
        }

        rect

    }

    fn rect_from_points(from: (i32, i32), to: (i32, i32)) -> Rect {
        let x1 = from.0.min(to.0);
        let y1 = from.1.min(to.1);
        let x2 = from.0.max(to.0);
        let y2 = from.1.max(to.1);

        Rect {x: x1, y: y1, w: x2 - x1, h: y2 - y1 }
    }

}

#[derive(Debug, Clone, Copy)]
pub struct Selection {
    rect: Rect,
    pub phase: SelectionPhase,
}

impl Selection {
    pub fn rect(&self) -> &Rect { &self.rect }

    pub fn idle() -> Self {
        Self {
            rect: Rect::zero(),
            phase: SelectionPhase::Idle,
        }
    }

    pub fn dragging(rect: Rect) -> Self {
        Self {
            rect,
            phase: SelectionPhase::Dragging,
        }
    }

    pub fn finalized(rect: Rect) -> Self {
        Self {
            rect,
            phase: SelectionPhase::Finalized,
        }
    }

    pub fn is_active(&self) -> bool {
        self.phase != SelectionPhase::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPhase {
    Idle,
    Dragging,
    Finalized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn zero() -> Self {
        Self {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    pub fn contains(&self, (x, y): (i32, i32)) -> bool {
        x >= self.x && x < self.x + self.w &&
        y >= self.y && y < self.y + self.h
    }

    pub fn as_f64(&self) -> (f64, f64, f64, f64) {
        (self.x as f64, self.y as f64, self.w as f64, self.h as f64)
    }

}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    None,
    Arrow,
    Rectangle,
    Blur,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionHitZone {
    Outside,
    Inside,

    N, S, E, W,
    NW, NE, SW, SE,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragMode {
    Create,
    Move,
    Resize(SelectionHitZone),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Arrow {
        from: (i32, i32),
        to: (i32, i32),
        color: (u8, u8, u8)
    },
    
    Rectangle {
        rect: Rect,
        color: (u8, u8, u8)
    },
    
    Blur {
        rect: Rect,
    },
}

impl Shape {
    pub fn is_valid(&self) -> bool {
        match self {
            Shape::Arrow { from, to, .. } => {
                let dist = ((to.0 - from.0).pow(2) + (to.1 - from.1).pow(2)).abs();
                dist > 10
            }
            Shape::Rectangle { rect, .. } | Shape::Blur { rect } => {
                rect.w > 5 && rect.h > 5
            }
        }
    }
}