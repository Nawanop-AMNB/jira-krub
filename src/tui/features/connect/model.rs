use crate::application::Credentials;
use crate::tui::widgets::TextInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Site,
    Email,
    Token,
    TestBtn,
    SaveBtn,
    QuitBtn,
}

impl Focus {
    pub const ORDER: [Focus; 6] = [Focus::Site, Focus::Email, Focus::Token, Focus::TestBtn, Focus::SaveBtn, Focus::QuitBtn];
    pub fn next(self) -> Self {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        Self::ORDER[(i + 1) % Self::ORDER.len()]
    }
    pub fn prev(self) -> Self {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        Self::ORDER[(i + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
    pub fn is_field(self) -> bool {
        matches!(self, Focus::Site | Focus::Email | Focus::Token)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestState {
    Idle,
    Testing,
    Ok { display_name: String, creds: Credentials },
    Err(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub site: TextInput,
    pub email: TextInput,
    pub token: TextInput,
    /// True when editing an existing config: token field shows •••• until retyped.
    pub token_kept: bool,
    pub kept_token: Option<String>,
    pub focus: Focus,
    pub test: TestState,
    pub site_err: Option<String>,
    pub email_err: Option<String>,
    pub token_err: Option<String>,
    pub banner: Option<String>,
    pub save_after_test: bool,
    pub had_config: bool,
    /// Opened from Settings → `Jira connection…`; Esc/Save return there.
    pub from_settings: bool,
}

impl Model {
    pub fn new(existing: Option<&Credentials>, banner: Option<String>) -> Self {
        let (site, email, kept) = match existing {
            Some(c) => (c.site.as_str().to_string(), c.email.clone(), Some(c.api_token.clone())),
            None => (String::new(), String::new(), None),
        };
        let focus = if existing.is_some() && banner.is_some() { Focus::Token } else { Focus::Site };
        Self {
            site: TextInput::with(site),
            email: TextInput::with(email),
            token: TextInput::default(),
            token_kept: kept.is_some() && banner.is_none(),
            kept_token: kept,
            focus,
            test: TestState::Idle,
            site_err: None,
            email_err: None,
            token_err: None,
            banner,
            save_after_test: false,
            had_config: existing.is_some(),
            from_settings: false,
        }
    }

    pub fn field_mut(&mut self) -> Option<&mut TextInput> {
        match self.focus {
            Focus::Site => Some(&mut self.site),
            Focus::Email => Some(&mut self.email),
            Focus::Token => Some(&mut self.token),
            _ => None,
        }
    }

    /// Effective token: typed, or the kept one.
    pub fn token_value(&self) -> String {
        if !self.token.is_empty() {
            self.token.text().to_string()
        } else if self.token_kept {
            self.kept_token.clone().unwrap_or_default()
        } else {
            String::new()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    FocusNext,
    FocusPrev,
    Focus(Focus),
    Char(char),
    Paste(String),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    ClearField,
    /// Enter: test on fields/test button, save on save button, quit on quit.
    Activate,
    Test,
    Save,
    Quit,
}
