use super::features::{connect, day, entry_form, settings, week};
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
    Connect(connect::Action),
    Settings(settings::Action),
    Week(week::Action),
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
