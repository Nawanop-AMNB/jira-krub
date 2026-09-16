//! Presentation. Feature-first: each screen/overlay owns model + keys + update + view.

pub mod action;
pub mod app;
pub mod deps;
pub mod features;
pub mod hit;
pub mod msg;
pub mod theme;
pub mod view;
pub mod widgets;

use anyhow::Result;
use app::App;
use crossterm::event::{self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use crossterm::execute;
use deps::Deps;
use std::io::stdout;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const TICK: Duration = Duration::from_millis(100);

fn restore() {
    if KITTY.load(Ordering::Relaxed) {
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }
    let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
    ratatui::restore();
}

/// Whether we pushed keyboard-enhancement flags (must be popped on exit).
static KITTY: AtomicBool = AtomicBool::new(false);

pub fn run(deps: Deps) -> Result<()> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        hook(info);
    }));

    let mut terminal = ratatui::init();
    execute!(stdout(), EnableMouseCapture, EnableBracketedPaste)?;
    // Kitty keyboard protocol (Ghostty, kitty, WezTerm, iTerm): lets
    // Ctrl+Enter / Shift+Enter reach us as distinct keys. Others ignore it.
    if matches!(crossterm::terminal::supports_keyboard_enhancement(), Ok(true))
        && execute!(stdout(), PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)).is_ok()
    {
        KITTY.store(true, Ordering::Relaxed);
    }
    let result = event_loop(&mut terminal, deps);
    restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, deps: Deps) -> Result<()> {
    let mut app = App::new(deps)?;
    loop {
        app.begin_tick();
        for msg in app.worker.drain() {
            app.on_msg(msg);
        }
        app.tick();

        let mut hits = hit::HitRegistry::default();
        terminal.draw(|f| view::draw(f, &app, &mut hits))?;
        app.hits = hits;

        if event::poll(TICK)? {
            match event::read()? {
                Event::Key(k) => app.on_key(k),
                Event::Mouse(m) => app.on_mouse(m),
                Event::Paste(s) => app.on_paste(s),
                _ => {}
            }
        }
        if app.quit {
            return Ok(());
        }
    }
}
#[cfg(test)]
pub mod test_support;
#[cfg(test)]
mod reducer_tests;
#[cfg(test)]
mod key_binding_tests;
#[cfg(test)]
mod push_flow_tests;
#[cfg(test)]
mod day_view_tests;
#[cfg(test)]
mod entry_form_tests;
