use crate::modules::screenshot::state::Tool;

pub enum AppAction {
    Screenshot(ScreenshotAction),
}

pub enum ScreenshotAction {
    SetTool(Tool),
    SetColor(u8, u8, u8),
    ToggleMode,
    MouseMove(i32, i32),
    DragBegin(i32, i32),
    DragUpdate(i32, i32),
    DragEnd,

    TextInput(char),
    TextBackspace,
    TextCommit,
    Escape,

    Save,
    Undo,
}
