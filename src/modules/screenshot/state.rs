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
    text_input: Option<TextInput>,
    screen_size: (i32, i32),
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
            current_shape: None,
            text_input: None,
            screen_size: (0, 0),
        }
    }
}

impl ScreenshotState {
    pub fn selection(&self) -> &Selection { &self.selection }
    pub fn is_paused(&self) -> bool { self.paused }
    pub fn mouse_pos(&self) -> (i32, i32) { self.mouse_pos }
    pub fn current_shape(&self) -> Option<&Shape> { self.current_shape.as_ref() }
    pub fn text_input(&self) -> Option<&TextInput> { self.text_input.as_ref() }
    pub fn screen_size(&self) -> (i32, i32) { self.screen_size }

    pub fn set_screen_size(&mut self, size: (i32, i32)) {
        self.screen_size = size;
    }

    pub fn toggle_pause(&mut self) {
        if self.selection.is_active() && !self.paused {
            self.paused = true;
        }
    }

    pub fn set_tool(&mut self, tool: Tool) {
        if self.current_tool == Tool::Text && tool != Tool::Text {
            self.text_input = None;
        }
        self.current_tool = tool;
    }

    pub fn set_color(&mut self, color: (u8, u8, u8)) {
        self.current_color = color;
    }

    pub fn begin_drag(&mut self, x: i32, y: i32) {
        self.mouse_pos = (x, y);

        if self.current_tool == Tool::Text {
            if self.paused
                && self.selection.is_active()
                && self.selection.rect.contains((x, y))
            {
                self.current_shape = None;
                self.drag_start = None;
                self.text_input = Some(TextInput {
                    position: (x, y),
                    text: String::new(),
                    color: self.current_color,
                });
            }
            return;
        }

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
            Some(10),
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

        let current = (start_x + dx, start_y + dy);

        self.selection.rect = match mode {
            DragMode::Create => Rect::from_points_bounded(
                (start_x, start_y),
                current,
                self.screen_size,
            ),
            DragMode::Move => origin.moved_by(dx, dy, self.screen_size),
            DragMode::Resize(zone) => origin.resized(zone, dx, dy, self.screen_size),
        };

        self.mouse_pos = current;
    }

    pub fn end_drag(&mut self) {
        self.drag_start = None;
        self.drag_origin = None;
        self.drag_mode = None;
        self.current_shape = None;

        if self.selection.is_active() && !self.selection.rect.is_empty() {
            self.selection = Selection::finalized(self.selection.rect);
        } else {
            self.selection = Selection::idle();
        }
    }

    pub fn set_mouse_pos(&mut self, pos: (i32, i32)) {
        self.mouse_pos = pos;
    }

    pub fn append_text(&mut self, ch: char) {
        if let Some(input) = self.text_input.as_mut() {
            input.text.push(ch);
        }
    }

    pub fn backspace_text(&mut self) {
        if let Some(input) = self.text_input.as_mut() {
            input.text.pop();
        }
    }

    pub fn cancel_text(&mut self) -> bool {
        self.text_input.take().is_some()
    }

    pub fn commit_text(&mut self) -> Option<Shape> {
        let input = self.text_input.take()?;
        if input.text.trim().is_empty() {
            return None;
        }

        Some(Shape::Text {
            position: input.position,
            text: input.text,
            color: input.color,
        })
    }

    pub fn export_selection(&self, original_surface: &ImageSurface) -> anyhow::Result<Vec<u8>> {
        export_selection(original_surface, self)
    }

    fn get_current_shape(&self) -> Option<Shape> {
        let from = self.drag_start?;
        let to = self.mouse_pos;

        match self.current_tool {
            Tool::Arrow => Some(Shape::Arrow {
                from,
                to,
                color: self.current_color,
            }),
            Tool::Rectangle => Some(Shape::Rectangle {
                rect: Self::rect_from_points(from, to),
                color: self.current_color,
            }),
            Tool::Blur => Some(Shape::Blur {
                rect: Self::rect_from_points(from, to),
            }),
            Tool::Text | Tool::None => None,
        }
    }

    fn rect_from_points(from: (i32, i32), to: (i32, i32)) -> Rect {
        Rect::from_points(from, to)
    }
}

#[derive(Debug, Clone)]
pub struct TextInput {
    position: (i32, i32),
    text: String,
    color: (u8, u8, u8),
}

impl TextInput {
    pub fn position(&self) -> (i32, i32) { self.position }
    pub fn text(&self) -> &str { &self.text }
    pub fn color(&self) -> (u8, u8, u8) { self.color }
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
        Self { x: 0, y: 0, w: 0, h: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    pub fn contains(&self, (x, y): (i32, i32)) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn as_f64(&self) -> (f64, f64, f64, f64) {
        (self.x as f64, self.y as f64, self.w as f64, self.h as f64)
    }

    fn right(&self) -> i32 {
        self.x + self.w
    }

    fn bottom(&self) -> i32 {
        self.y + self.h
    }

    fn from_points(from: (i32, i32), to: (i32, i32)) -> Self {
        let left = from.0.min(to.0);
        let top = from.1.min(to.1);
        let right = from.0.max(to.0);
        let bottom = from.1.max(to.1);

        Self::from_edges(left, top, right, bottom)
    }

    fn from_points_bounded(
        from: (i32, i32),
        to: (i32, i32),
        screen_size: (i32, i32),
    ) -> Self {
        let (screen_w, screen_h) = screen_size;
        let max_x = screen_w.max(0);
        let max_y = screen_h.max(0);

        let from = (from.0.clamp(0, max_x), from.1.clamp(0, max_y));
        let to = (to.0.clamp(0, max_x), to.1.clamp(0, max_y));

        Self::from_points(from, to)
    }

    fn from_edges(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            x: left,
            y: top,
            w: right - left,
            h: bottom - top,
        }
    }

    fn moved_by(self, dx: i32, dy: i32, screen_size: (i32, i32)) -> Self {
        let (screen_w, screen_h) = screen_size;
        let max_x = (screen_w - self.w).max(0);
        let max_y = (screen_h - self.h).max(0);

        Self {
            x: (self.x + dx).clamp(0, max_x),
            y: (self.y + dy).clamp(0, max_y),
            ..self
        }
    }

    fn resized(
        self,
        zone: SelectionHitZone,
        dx: i32,
        dy: i32,
        screen_size: (i32, i32),
    ) -> Self {
        let screen_w = screen_size.0.max(1);
        let screen_h = screen_size.1.max(1);

        let mut left = self.x.clamp(0, screen_w - 1);
        let mut top = self.y.clamp(0, screen_h - 1);
        let mut right = self.right().clamp(left + 1, screen_w);
        let mut bottom = self.bottom().clamp(top + 1, screen_h);

        if matches!(zone, SelectionHitZone::W | SelectionHitZone::NW | SelectionHitZone::SW) {
            left = (self.x + dx).clamp(0, right - 1);
        }

        if matches!(zone, SelectionHitZone::E | SelectionHitZone::NE | SelectionHitZone::SE) {
            right = (self.right() + dx).clamp(left + 1, screen_w);
        }

        if matches!(zone, SelectionHitZone::N | SelectionHitZone::NW | SelectionHitZone::NE) {
            top = (self.y + dy).clamp(0, bottom - 1);
        }

        if matches!(zone, SelectionHitZone::S | SelectionHitZone::SW | SelectionHitZone::SE) {
            bottom = (self.bottom() + dy).clamp(top + 1, screen_h);
        }

        Self::from_edges(left, top, right, bottom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    None,
    Arrow,
    Rectangle,
    Text,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    Arrow {
        from: (i32, i32),
        to: (i32, i32),
        color: (u8, u8, u8),
    },
    Rectangle {
        rect: Rect,
        color: (u8, u8, u8),
    },
    Text {
        position: (i32, i32),
        text: String,
        color: (u8, u8, u8),
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
            Shape::Text { text, .. } => !text.trim().is_empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_rect(rect: Rect, screen_size: (i32, i32)) -> ScreenshotState {
        ScreenshotState {
            selection: Selection::dragging(rect),
            screen_size,
            ..ScreenshotState::default()
        }
    }

    fn paused_state_with_rect(rect: Rect, screen_size: (i32, i32)) -> ScreenshotState {
        ScreenshotState {
            selection: Selection::finalized(rect),
            paused: true,
            screen_size,
            ..ScreenshotState::default()
        }
    }

    #[test]
    fn create_drag_clamps_current_point_at_top_left() {
        let mut state = ScreenshotState::default();
        state.set_screen_size((200, 200));
        state.drag_start = Some((50, 60));
        state.drag_origin = Some(Rect::zero());
        state.drag_mode = Some(DragMode::Create);
        state.selection = Selection::dragging(Rect::zero());

        state.update_drag(-100, -100);

        assert_eq!(*state.selection().rect(), Rect { x: 0, y: 0, w: 50, h: 60 });
    }

    #[test]
    fn move_clamps_without_changing_size() {
        let origin = Rect { x: 20, y: 30, w: 80, h: 60 };
        let mut state = state_with_rect(origin, (200, 200));
        state.drag_start = Some((40, 50));
        state.drag_origin = Some(origin);
        state.drag_mode = Some(DragMode::Move);

        state.update_drag(500, 500);

        assert_eq!(*state.selection().rect(), Rect { x: 120, y: 140, w: 80, h: 60 });
    }

    #[test]
    fn resize_nw_clamps_to_screen_and_keeps_south_east_fixed() {
        let origin = Rect { x: 20, y: 30, w: 100, h: 80 };
        let mut state = state_with_rect(origin, (300, 300));
        state.drag_start = Some((20, 30));
        state.drag_origin = Some(origin);
        state.drag_mode = Some(DragMode::Resize(SelectionHitZone::NW));

        state.update_drag(-50, -60);

        assert_eq!(*state.selection().rect(), Rect { x: 0, y: 0, w: 120, h: 110 });
    }

    #[test]
    fn resize_w_cannot_cross_fixed_east_edge() {
        let origin = Rect { x: 20, y: 30, w: 100, h: 80 };
        let mut state = state_with_rect(origin, (300, 300));
        state.drag_start = Some((20, 50));
        state.drag_origin = Some(origin);
        state.drag_mode = Some(DragMode::Resize(SelectionHitZone::W));

        state.update_drag(200, 0);

        assert_eq!(*state.selection().rect(), Rect { x: 119, y: 30, w: 1, h: 80 });
    }

    #[test]
    fn resize_se_clamps_to_screen_edges() {
        let origin = Rect { x: 20, y: 30, w: 100, h: 80 };
        let mut state = state_with_rect(origin, (200, 180));
        state.drag_start = Some((120, 110));
        state.drag_origin = Some(origin);
        state.drag_mode = Some(DragMode::Resize(SelectionHitZone::SE));

        state.update_drag(500, 500);

        assert_eq!(*state.selection().rect(), Rect { x: 20, y: 30, w: 180, h: 150 });
    }

    #[test]
    fn text_tool_click_starts_text_input_inside_selection() {
        let rect = Rect { x: 20, y: 30, w: 120, h: 80 };
        let mut state = paused_state_with_rect(rect, (200, 200));
        state.set_tool(Tool::Text);

        state.begin_drag(40, 50);

        let input = state.text_input().expect("text input should start");
        assert_eq!(input.position(), (40, 50));
        assert_eq!(input.text(), "");
    }

    #[test]
    fn text_input_supports_edit_commit_and_cancel() {
        let rect = Rect { x: 20, y: 30, w: 120, h: 80 };
        let mut state = paused_state_with_rect(rect, (200, 200));
        state.set_tool(Tool::Text);
        state.begin_drag(40, 50);

        state.append_text('Ж');
        state.append_text('a');
        state.backspace_text();

        let shape = state.commit_text().expect("non-empty text should commit");
        assert!(matches!(shape, Shape::Text { ref text, .. } if text == "Ж"));
        assert!(state.text_input().is_none());

        state.begin_drag(60, 70);
        assert!(state.cancel_text());
        assert!(state.text_input().is_none());
    }
}
