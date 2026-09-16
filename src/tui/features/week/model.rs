use crate::domain::Week;
use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub week: Week,
    pub selected: usize,
}

impl Model {
    pub fn new(today: NaiveDate) -> Self {
        let week = Week::containing(today);
        Self { week, selected: week.index_of(today).unwrap_or(0) }
    }
    pub fn selected_date(&self) -> NaiveDate {
        self.week.days()[self.selected.min(6)]
    }
    pub fn select_date(&mut self, d: NaiveDate) {
        if let Some(i) = self.week.index_of(d) {
            self.selected = i;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    PrevWeek,
    NextWeek,
    Today,
    Up,
    Down,
    Select(usize),
    OpenDay(usize),
    AddEntry,
    PushWeek,
}
