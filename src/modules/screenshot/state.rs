use cairo::{Context, Format, ImageSurface};

use crate::capture::screenshot::export::export_selection;
use crate::common::cursor;
use crate::modules::screenshot::render;

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

    pub fn set_tool(&mut self, tool: Tool) -> Option<Shape> {
        let pending_text = if self.current_tool == Tool::Text && tool != Tool::Text {
            self.commit_text()
        } else {
            None
        };

        self.current_tool = tool;
        pending_text
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
                self.text_input = Some(TextInput::new((x, y)));
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
        let current = (start_x + dx, start_y + dy);

        if self.current_tool != Tool::None {
            self.current_shape = self.get_current_shape(current);
            self.mouse_pos = current;
            return;
        }

        let Some(origin) = self.drag_origin else { return; };
        let Some(mode) = self.drag_mode else { return; };

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
        let selection = self.selection.rect;
        let color = self.current_color;
        let Some(input) = self.text_input.as_mut() else { return; };

        let mut candidate = input.clone();
        candidate.append(ch, color);

        if Self::text_input_fits_selection(&candidate, &selection) {
            *input = candidate;
        }
    }

    fn text_input_fits_selection(input: &TextInput, selection: &Rect) -> bool {
        let Ok(surface) = ImageSurface::create(Format::ARgb32, 1, 1) else {
            return false;
        };
        let Ok(cr) = Context::new(&surface) else {
            return false;
        };

        // Reuse the renderer's font setup so input validation matches the
        // actual preview/commit typography.
        if render::text_bounds(&cr, input.position, &input.runs).is_none() {
            return false;
        }

        let mut cursor_x = input.position.0 as f64;
        let mut right = cursor_x;

        for run in &input.runs {
            let Ok(extents) = cr.text_extents(run.text()) else {
                return false;
            };

            right = right.max(
                cursor_x + extents.x_bearing() + extents.width()
            );
            cursor_x += extents.x_advance();
        }

        // Ink extents ignore trailing spaces; the caret position does not.
        // Guard the caret too so invisible whitespace cannot accumulate
        // beyond the selection's right edge.
        right = right.max(cursor_x + 1.0);

        right <= selection.right() as f64
    }

    pub fn backspace_text(&mut self) {
        if let Some(input) = self.text_input.as_mut() {
            input.backspace();
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
            runs: input.runs,
        })
    }

    pub fn export_selection(&self, original_surface: &ImageSurface) -> anyhow::Result<Vec<u8>> {
        export_selection(original_surface, self)
    }

    fn get_current_shape(&self, to: (i32, i32)) -> Option<Shape> {
        let from = self.drag_start?;

        match self.current_tool {
            Tool::Arrow => self.constrained_arrow(from, to),
            Tool::Rectangle => {
                let safe = self.selection.rect.inset(
                    render::stroke_padding(render::RECTANGLE_LINE_WIDTH)
                )?;
                let from = safe.clamp_point(from);
                let to = safe.clamp_point(to);

                Some(Shape::Rectangle {
                    rect: Self::rect_from_points(from, to),
                    color: self.current_color,
                })
            }
            Tool::Blur => {
                let to = self.selection.rect.clamp_point(to);
                Some(Shape::Blur {
                    rect: Self::rect_from_points(from, to),
                })
            }
            Tool::Text | Tool::None => None,
        }
    }

    fn constrained_arrow(
        &self,
        raw_from: (i32, i32),
        raw_to: (i32, i32),
    ) -> Option<Shape> {
        let safe = self.selection.rect.inset(
            render::stroke_padding(render::ARROW_LINE_WIDTH)
        )?;
        let from = safe.clamp_point(raw_from);

        let raw_dx = (raw_to.0 - raw_from.0) as f64;
        let raw_dy = (raw_to.1 - raw_from.1) as f64;
        let raw_length = raw_dx.hypot(raw_dy);
        if raw_length <= f64::EPSILON {
            return None;
        }

        let direction = (raw_dx / raw_length, raw_dy / raw_length);
        let angle = raw_dy.atan2(raw_dx);
        let offsets = render::arrow_head_offsets(angle);
        let tip_bounds = Self::admissible_tip_bounds(&safe, offsets)?;
        let (entry, exit) = Self::ray_box_interval(from, direction, tip_bounds)?;

        let mut distance = raw_length.min(exit);
        if distance + 1e-9 < entry {
            return None;
        }

        // Rounding the ray point to integer canvas coordinates can slightly
        // rotate the final segment. Re-check the exact renderer geometry and
        // retreat inward along the same intended ray when necessary.
        for _ in 0..=64 {
            if distance + 1e-9 < entry {
                break;
            }

            let to = (
                (from.0 as f64 + direction.0 * distance).round() as i32,
                (from.1 as f64 + direction.1 * distance).round() as i32,
            );

            if to != from && Self::arrow_geometry_fits(from, to, &safe) {
                return Some(Shape::Arrow {
                    from,
                    to,
                    color: self.current_color,
                });
            }

            let next = (distance - 0.25).max(entry);
            if (next - distance).abs() <= f64::EPSILON {
                break;
            }
            distance = next;
        }

        None
    }

    fn admissible_tip_bounds(
        safe: &Rect,
        head_offsets: [(f64, f64); 2],
    ) -> Option<(f64, f64, f64, f64)> {
        let offsets = [(0.0, 0.0), head_offsets[0], head_offsets[1]];
        let mut min_x = safe.x as f64;
        let mut max_x = safe.right() as f64;
        let mut min_y = safe.y as f64;
        let mut max_y = safe.bottom() as f64;

        for (offset_x, offset_y) in offsets {
            min_x = min_x.max(safe.x as f64 - offset_x);
            max_x = max_x.min(safe.right() as f64 - offset_x);
            min_y = min_y.max(safe.y as f64 - offset_y);
            max_y = max_y.min(safe.bottom() as f64 - offset_y);
        }

        if min_x > max_x || min_y > max_y {
            return None;
        }

        Some((min_x, max_x, min_y, max_y))
    }

    fn ray_box_interval(
        origin: (i32, i32),
        direction: (f64, f64),
        bounds: (f64, f64, f64, f64),
    ) -> Option<(f64, f64)> {
        let (min_x, max_x, min_y, max_y) = bounds;
        let mut entry: f64 = 0.0;
        let mut exit = f64::INFINITY;

        for (origin, direction, min, max) in [
            (origin.0 as f64, direction.0, min_x, max_x),
            (origin.1 as f64, direction.1, min_y, max_y),
        ] {
            if direction.abs() <= f64::EPSILON {
                if origin < min || origin > max {
                    return None;
                }
                continue;
            }

            let mut near = (min - origin) / direction;
            let mut far = (max - origin) / direction;
            if near > far {
                std::mem::swap(&mut near, &mut far);
            }

            entry = entry.max(near);
            exit = exit.min(far);
            if entry > exit {
                return None;
            }
        }

        if exit < 0.0 {
            return None;
        }

        Some((entry.max(0.0), exit))
    }

    fn arrow_geometry_fits(
        from: (i32, i32),
        to: (i32, i32),
        safe: &Rect,
    ) -> bool {
        if !safe.contains_closed((from.0 as f64, from.1 as f64))
            || !safe.contains_closed((to.0 as f64, to.1 as f64))
        {
            return false;
        }

        render::arrow_head_points(from, to)
            .into_iter()
            .all(|point| safe.contains_closed(point))
    }

    fn rect_from_points(from: (i32, i32), to: (i32, i32)) -> Rect {
        Rect::from_points(from, to)
    }
}

#[derive(Debug, Clone)]
pub struct TextInput {
    position: (i32, i32),
    text: String,
    runs: Vec<TextRun>,
}

impl TextInput {
    fn new(position: (i32, i32)) -> Self {
        Self {
            position,
            text: String::new(),
            runs: Vec::new(),
        }
    }

    fn append(&mut self, ch: char, color: (u8, u8, u8)) {
        self.text.push(ch);

        if let Some(run) = self.runs.last_mut() {
            if run.color == color {
                run.text.push(ch);
                return;
            }
        }

        self.runs.push(TextRun::new(ch.to_string(), color));
    }

    fn backspace(&mut self) {
        if self.text.pop().is_none() {
            return;
        }

        let remove_last_run = if let Some(run) = self.runs.last_mut() {
            run.text.pop();
            run.text.is_empty()
        } else {
            false
        };

        if remove_last_run {
            self.runs.pop();
        }
    }

    pub fn position(&self) -> (i32, i32) { self.position }
    pub fn text(&self) -> &str { &self.text }
    pub fn runs(&self) -> &[TextRun] { &self.runs }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRun {
    text: String,
    color: (u8, u8, u8),
}

impl TextRun {
    pub fn new(text: String, color: (u8, u8, u8)) -> Self {
        Self { text, color }
    }

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

    fn clamp_point(&self, (x, y): (i32, i32)) -> (i32, i32) {
        (
            x.clamp(self.x, self.right()),
            y.clamp(self.y, self.bottom()),
        )
    }

    fn contains_closed(&self, (x, y): (f64, f64)) -> bool {
        x >= self.x as f64
            && x <= self.right() as f64
            && y >= self.y as f64
            && y <= self.bottom() as f64
    }

    fn inset(&self, padding: i32) -> Option<Self> {
        let padding = padding.max(0);
        if self.w < padding * 2 || self.h < padding * 2 {
            return None;
        }

        Some(Self {
            x: self.x + padding,
            y: self.y + padding,
            w: self.w - padding * 2,
            h: self.h - padding * 2,
        })
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
        runs: Vec<TextRun>,
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
            Shape::Text { runs, .. } => runs.iter().any(|run| !run.text().trim().is_empty()),
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

    fn rendered_shape_png(shape: &Shape, clip: Option<&Rect>) -> Vec<u8> {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 160).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint().unwrap();

        if let Some(clip) = clip {
            render::draw_shape_clipped(&surface, &cr, shape, clip);
        } else {
            render::draw_shape(&surface, &cr, shape);
        }

        let mut bytes = Vec::new();
        surface.write_to_png(&mut bytes).unwrap();
        bytes
    }

    fn assert_render_fits_selection(shape: &Shape, selection: &Rect) {
        assert_eq!(
            rendered_shape_png(shape, None),
            rendered_shape_png(shape, Some(selection)),
        );
    }

    #[test]
    fn arrow_drag_toward_each_edge_is_fully_rendered_inside_selection() {
        let rect = Rect { x: 20, y: 20, w: 120, h: 100 };
        let start = (80, 70);

        for target in [(80, -100), (80, 300), (-100, 70), (300, 70)] {
            let mut state = paused_state_with_rect(rect, (200, 160));
            let _ = state.set_tool(Tool::Arrow);
            state.begin_drag(start.0, start.1);
            state.update_drag(target.0 - start.0, target.1 - start.1);

            let shape = state.current_shape().expect("contained arrow should exist");
            assert_render_fits_selection(shape, &rect);
        }
    }

    #[test]
    fn diagonal_arrow_drag_preserves_direction_and_rendered_bounds() {
        let rect = Rect { x: 20, y: 20, w: 120, h: 100 };
        let start = (80, 70);

        for target in [
            (-100, -100),
            (300, -100),
            (-100, 300),
            (300, 300),
        ] {
            let mut state = paused_state_with_rect(rect, (200, 160));
            let _ = state.set_tool(Tool::Arrow);
            state.begin_drag(start.0, start.1);
            state.update_drag(target.0 - start.0, target.1 - start.1);

            let shape = state.current_shape().expect("contained arrow should exist");
            assert_render_fits_selection(shape, &rect);

            let Shape::Arrow { from, to, .. } = shape else {
                panic!("expected arrow");
            };
            let raw = ((target.0 - start.0) as f64, (target.1 - start.1) as f64);
            let actual = ((to.0 - from.0) as f64, (to.1 - from.1) as f64);
            let raw_len = raw.0.hypot(raw.1);
            let actual_len = actual.0.hypot(actual.1);
            let cross = (raw.0 * actual.1 - raw.1 * actual.0).abs();
            let dot = raw.0 * actual.0 + raw.1 * actual.1;

            assert!(dot > 0.0, "arrow reversed direction");
            assert!(
                cross / (raw_len * actual_len) <= 0.03,
                "arrow direction changed beyond integer-rounding tolerance"
            );
        }
    }

    #[test]
    fn rectangle_drag_toward_each_edge_is_fully_rendered_inside_selection() {
        let rect = Rect { x: 20, y: 20, w: 120, h: 100 };
        let start = (80, 70);

        for target in [(110, -100), (110, 300), (-100, 90), (300, 90)] {
            let mut state = paused_state_with_rect(rect, (200, 160));
            let _ = state.set_tool(Tool::Rectangle);
            state.begin_drag(start.0, start.1);
            state.update_drag(target.0 - start.0, target.1 - start.1);

            let shape = state.current_shape().expect("contained rectangle should exist");
            assert_render_fits_selection(shape, &rect);
        }
    }

    #[test]
    fn annotation_drag_cannot_start_outside_selection() {
        let rect = Rect { x: 20, y: 30, w: 120, h: 80 };
        let mut state = paused_state_with_rect(rect, (200, 200));
        let _ = state.set_tool(Tool::Arrow);

        state.begin_drag(10, 10);
        state.update_drag(100, 100);

        assert!(state.current_shape().is_none());
    }

    #[test]
    fn blur_drag_stops_at_selection_boundary() {
        let rect = Rect { x: 20, y: 30, w: 120, h: 80 };
        let mut state = paused_state_with_rect(rect, (200, 200));
        let _ = state.set_tool(Tool::Blur);
        state.begin_drag(100, 90);

        state.update_drag(-500, -500);

        assert_eq!(
            state.current_shape(),
            Some(&Shape::Blur {
                rect: Rect { x: 20, y: 30, w: 80, h: 60 },
            })
        );
    }

    #[test]
    fn text_tool_click_starts_text_input_inside_selection() {
        let rect = Rect { x: 20, y: 30, w: 120, h: 80 };
        let mut state = paused_state_with_rect(rect, (200, 200));
        let _ = state.set_tool(Tool::Text);

        state.begin_drag(40, 50);

        let input = state.text_input().expect("text input should start");
        assert_eq!(input.position(), (40, 50));
        assert_eq!(input.text(), "");
    }

    #[test]
    fn text_input_supports_edit_commit_and_cancel() {
        let rect = Rect { x: 20, y: 30, w: 120, h: 80 };
        let mut state = paused_state_with_rect(rect, (200, 200));
        let _ = state.set_tool(Tool::Text);
        state.begin_drag(40, 50);

        state.append_text('Ж');
        state.append_text('a');
        state.backspace_text();

        let shape = state.commit_text().expect("non-empty text should commit");
        assert!(matches!(
            shape,
            Shape::Text { ref runs, .. }
                if runs.len() == 1 && runs[0].text() == "Ж"
        ));
        assert!(state.text_input().is_none());

        state.begin_drag(60, 70);
        assert!(state.cancel_text());
        assert!(state.text_input().is_none());
    }

    #[test]
    fn text_input_stops_before_crossing_selection_right_edge() {
        let rect = Rect { x: 20, y: 30, w: 120, h: 80 };
        let mut state = paused_state_with_rect(rect, (200, 200));
        let _ = state.set_tool(Tool::Text);
        state.begin_drag(40, 50);

        for _ in 0..100 {
            state.append_text('W');
        }

        let stopped = state.text_input().unwrap().text().to_string();
        assert!(!stopped.is_empty());
        assert!(stopped.len() < 100);

        state.append_text('W');
        assert_eq!(state.text_input().unwrap().text(), stopped);
    }

    #[test]
    fn trailing_spaces_cannot_continue_past_selection_right_edge() {
        let rect = Rect { x: 20, y: 30, w: 120, h: 80 };
        let mut state = paused_state_with_rect(rect, (200, 200));
        let _ = state.set_tool(Tool::Text);
        state.begin_drag(100, 50);

        for _ in 0..100 {
            state.append_text(' ');
        }

        let stopped = state.text_input().unwrap().text().to_string();
        assert!(stopped.len() < 100);

        state.append_text(' ');
        assert_eq!(state.text_input().unwrap().text(), stopped);
    }

    #[test]
    fn changing_color_only_affects_future_text() {
        let rect = Rect { x: 20, y: 30, w: 120, h: 80 };
        let mut state = paused_state_with_rect(rect, (200, 200));
        let _ = state.set_tool(Tool::Text);
        state.begin_drag(40, 50);
        state.append_text('A');

        assert_eq!(state.text_input().unwrap().runs()[0].color(), (255, 0, 0));

        state.set_color((12, 34, 56));

        assert_eq!(state.text_input().unwrap().runs()[0].color(), (255, 0, 0));

        state.append_text('B');

        let input = state.text_input().unwrap();
        assert_eq!(input.text(), "AB");
        assert_eq!(input.runs().len(), 2);
        assert_eq!(input.runs()[0].text(), "A");
        assert_eq!(input.runs()[0].color(), (255, 0, 0));
        assert_eq!(input.runs()[1].text(), "B");
        assert_eq!(input.runs()[1].color(), (12, 34, 56));

        let shape = state.commit_text().expect("colored text should commit");
        assert!(matches!(
            shape,
            Shape::Text { ref runs, .. }
                if runs.len() == 2
                    && runs[0].text() == "A"
                    && runs[0].color() == (255, 0, 0)
                    && runs[1].text() == "B"
                    && runs[1].color() == (12, 34, 56)
        ));
    }

    #[test]
    fn backspace_removes_text_across_color_run_boundary() {
        let rect = Rect { x: 20, y: 30, w: 120, h: 80 };
        let mut state = paused_state_with_rect(rect, (200, 200));
        let _ = state.set_tool(Tool::Text);
        state.begin_drag(40, 50);
        state.append_text('A');
        state.set_color((12, 34, 56));
        state.append_text('B');

        state.backspace_text();

        let input = state.text_input().unwrap();
        assert_eq!(input.text(), "A");
        assert_eq!(input.runs().len(), 1);
        assert_eq!(input.runs()[0].text(), "A");
        assert_eq!(input.runs()[0].color(), (255, 0, 0));
    }

    #[test]
    fn switching_away_from_text_returns_pending_text_shape() {
        let rect = Rect { x: 20, y: 30, w: 120, h: 80 };
        let mut state = paused_state_with_rect(rect, (200, 200));
        let _ = state.set_tool(Tool::Text);
        state.begin_drag(40, 50);
        state.append_text('A');

        let shape = state.set_tool(Tool::Arrow)
            .expect("switching tools should preserve typed text");

        assert!(matches!(
            shape,
            Shape::Text { ref runs, .. }
                if runs.len() == 1 && runs[0].text() == "A"
        ));
        assert!(state.text_input().is_none());
    }
}
