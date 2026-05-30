use shinra_tui::config;
use shinra_tui::tui;

fn main() -> anyhow::Result<()> {
    // Redirect stderr to a log file so Mesa/EGL GPU driver warnings
    // don't corrupt the TUI display.
    #[cfg(unix)]
    {
        use std::fs::File;
        use std::os::unix::io::AsRawFd;
        if let Ok(log) = File::create("/tmp/shinra-ide-stderr.log") {
            unsafe { libc::dup2(log.as_raw_fd(), 2); }
        }
    }

    let config = config::Config::load()?;
    match config.mode {
        config::Mode::Tui => tui::run(config),
        config::Mode::Gui => {
            Ok(())
        }
    }
}
