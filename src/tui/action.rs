use super::features::{connect, day, entry_form, settings, tasks, week};
use crate::domain::Week;
use chrono::NaiveDate;

/// Every user intent, from keys or mouse. Features own their sub-enums.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Nop,
    Quit,
    ForceQuit,
    Refresh,
    OpenSettings,
    /// `Tab` on the main screen: toggle Worklog ↔ My tasks.
    SwitchMainTab,
    /// Clicking a main-screen tab label.
    SetMainTab(crate::tui::app::MainTab),
    Connect(connect::Action),
    Settings(settings::Action),
    Week(week::Action),
    Tasks(tasks::Action),
    Day(day::Action),
    Form(entry_form::Action),
    ConfirmYes,
    ConfirmNo,
    PushRequest(PushScope),
    PushConfirmed(PushScope),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushScope {
    Day(NaiveDate),
    Week(Week),
}
