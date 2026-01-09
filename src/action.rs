use crate::modules::screenshot::state::Tool;

pub enum AppAction {
    Screenshot(ScreenshotAction),
    Global(GlobalAction)
}

pub enum ScreenshotAction {
    SetTool(Tool),
    SetColor(u8, u8, u8),
    ToogleMode,
    MouseMove(i32, i32),
    DragBegin(i32, i32),
    DragUpdate(i32, i32),
    DragEnd,

    Save,
}

pub enum GlobalAction {
    Quit
}