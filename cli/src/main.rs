fn main() -> std::process::ExitCode {
    restore_default_sigpipe();
    omatheme::cli::run()
}

/// Rust ignores SIGPIPE, so printing into a closed pipe (`omatheme list | head`) panics. Restore
/// the default, so the process ends quietly like other command-line tools.
#[cfg(unix)]
fn restore_default_sigpipe() {
    unsafe extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
    }
    const SIGPIPE: i32 = 13;
    const SIG_DFL: usize = 0;
    // SAFETY: setting a signal's disposition to the default before any threads start.
    unsafe {
        signal(SIGPIPE, SIG_DFL);
    }
}

#[cfg(not(unix))]
fn restore_default_sigpipe() {}
