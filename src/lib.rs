//! `sloppytoppy`: a terminal dashboard for running Claude and Codex processes.
//!
//! The crate is split so each concern is testable on its own:
//!
//! * [`process`]: discover, sample (on a background thread) and safely signal processes.
//! * [`usage`]: parse the CLIs' session logs into token counts.
//! * [`app`] / [`input`]: UI state and key bindings, free of rendering.
//! * [`ui`]: stateless ratatui rendering.

pub mod app;
pub mod cli;
pub mod error;
pub mod input;
pub mod process;
pub mod report;
pub mod series;
pub mod ui;
pub mod usage;

use std::io::{self, Write};
use std::thread;

pub use cli::Cli;
pub use error::{Error, Result};

use app::App;
use process::Sampler;

/// Runs the program described by `cli`.
///
/// # Errors
/// Returns an error on terminal or sampler-thread I/O failure.
pub fn run(cli: &Cli) -> Result<()> {
    if cli.once {
        return run_once(cli);
    }
    let sampler = process::spawn(cli.interval(), Sampler::new(!cli.no_usage))?;
    let mut app = App::new(Some(sampler));
    ratatui::run(|terminal| app.run(terminal))?;
    Ok(())
}

/// Prints one snapshot. CPU needs two samples to be meaningful, hence the pause.
fn run_once(cli: &Cli) -> Result<()> {
    let mut sampler = Sampler::new(!cli.no_usage);
    sampler.sample();
    thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    let snapshot = sampler.sample();
    let mut out = io::stdout().lock();
    report::write_table(&mut out, &snapshot)?;
    out.flush()?;
    Ok(())
}
