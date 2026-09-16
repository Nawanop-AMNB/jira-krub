use super::action::Action;
use crate::domain::IssueKey;
use ratatui::layout::{Position, Rect};

/// One clickable region, registered during draw. Later registrations win
/// (overlays are drawn last, so they take precedence).
#[derive(Default)]
pub struct HitArea {
    pub rect: Rect,
    pub click: Option<Action>,
    pub double: Option<Action>,
    /// Click with the column offset inside the rect (for text cursors).
    pub click_at: Option<Box<dyn Fn(u16) -> Action>>,
    pub scroll_up: Option<Action>,
    pub scroll_down: Option<Action>,
    /// Mouse-down here starts dragging this issue.
    pub drag: Option<IssueKey>,
    /// Mouse-up here after a drag stages the dragged issue.
    pub drop_target: bool,
}

#[derive(Default)]
pub struct HitRegistry {
    areas: Vec<HitArea>,
}

impl HitRegistry {
    pub fn add(&mut self, area: HitArea) {
        self.areas.push(area);
    }
    pub fn click(&mut self, rect: Rect, action: Action) {
        self.add(HitArea { rect, click: Some(action), ..Default::default() });
    }
    pub fn at(&self, pos: Position) -> Option<&HitArea> {
        self.areas.iter().rev().find(|a| a.rect.contains(pos))
    }
    /// Innermost area at `pos` that has the given capability.
    pub fn find<F: Fn(&HitArea) -> bool>(&self, pos: Position, pred: F) -> Option<&HitArea> {
        self.areas.iter().rev().find(|a| a.rect.contains(pos) && pred(a))
    }
}
