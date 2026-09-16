use ratatui::style::{Color, Modifier, Style};

pub fn accent() -> Style {
    Style::new().fg(Color::Cyan)
}
pub fn title() -> Style {
    Style::new().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
}
pub fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}
pub fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}
pub fn good() -> Style {
    Style::new().fg(Color::Green)
}
pub fn warn() -> Style {
    Style::new().fg(Color::Yellow)
}
pub fn bad() -> Style {
    Style::new().fg(Color::Red)
}
pub fn selected() -> Style {
    Style::new().add_modifier(Modifier::REVERSED)
}
pub fn key_hint() -> Style {
    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}
pub fn editable() -> Style {
    Style::new().add_modifier(Modifier::UNDERLINED | Modifier::DIM)
}
pub fn editing() -> Style {
    Style::new().fg(Color::Black).bg(Color::Yellow)
}
pub fn editing_error() -> Style {
    Style::new().fg(Color::White).bg(Color::Red)
}
pub fn issue_key() -> Style {
    Style::new().fg(Color::Blue)
}
pub fn watched() -> Style {
    Style::new().fg(Color::Magenta)
}
