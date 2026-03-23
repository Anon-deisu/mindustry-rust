#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Fire,
    Use,
    Pause,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerIntent {
    SetMoveAxis { x: f32, y: f32 },
    SetAimAxis { x: f32, y: f32 },
    ActionPressed(BinaryAction),
    ActionHeld(BinaryAction),
    ActionReleased(BinaryAction),
}
