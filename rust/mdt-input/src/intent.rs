#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Fire,
    Boost,
    Chat,
    Interact,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerIntent {
    SetMoveAxis { x: f32, y: f32 },
    SetAimAxis { x: f32, y: f32 },
    SetMiningTile { tile: Option<(i32, i32)> },
    SetBuilding { building: bool },
    ConfigTap { tile: (i32, i32) },
    ActionPressed(BinaryAction),
    ActionHeld(BinaryAction),
    ActionReleased(BinaryAction),
}
