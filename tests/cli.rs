//! The command-line surface, driven as a real process.
//!
//! Calling the handler directly would prove nothing about the argument
//! parsing, the exit code or the split between stdout and stderr, so every
//! test here spawns the built binary and inspects all three.
//!
//! The interactive calculator itself is not started here: it takes over the
//! terminal and would block. Only the paths that print and exit are covered.

use std::process::{Command, Output};

/// Runs the built binary with `args` and returns its captured output.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_calcli"))
        .args(args)
        .output()
        .expect("the calcli binary is built before its integration tests")
}

/// The exit code, which must be observable: a `None` means the process was
/// killed by a signal, and that is a failure in itself.
fn code(output: &Output) -> i32 {
    output.status.code().expect("calcli exited on its own")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn help_goes_to_stdout_and_succeeds() {
    for flag in ["-h", "--help"] {
        let output = run(&[flag]);
        assert_eq!(code(&output), 0, "{flag} must succeed");
        assert!(stderr(&output).is_empty(), "{flag} must not use stderr");

        let text = stdout(&output);
        assert!(text.contains("Usage:"), "{flag} lacks a usage line: {text}");
        assert!(text.contains("--demo"), "{flag} omits an option: {text}");
        assert!(text.contains("--help"), "{flag} omits itself: {text}");
        assert!(text.contains("--version"), "{flag} omits --version: {text}");
    }
}

/// Name and version on one line, nothing else - that is the whole contract.
#[test]
fn version_prints_the_name_and_version_only() {
    for flag in ["-V", "--version"] {
        let output = run(&[flag]);
        assert_eq!(code(&output), 0, "{flag} must succeed");

        let text = stdout(&output);
        assert_eq!(text.lines().count(), 1, "{flag} printed more than a line");
        assert!(text.starts_with("calcli "), "{flag}: {text}");
        assert!(
            text.trim().ends_with(env!("CARGO_PKG_VERSION")),
            "{flag} reports a version other than the crate's: {text}",
        );
    }
}

/// An unknown option is a usage error, and a usage error is exit 2. Getting
/// this wrong is silent: a calling script checking `$?` would read 1 as an
/// ordinary runtime failure and 0 as success.
#[test]
fn an_unknown_option_is_a_usage_error_on_stderr() {
    let output = run(&["--definitely-not-an-option"]);

    assert_eq!(code(&output), 2, "a usage error must exit 2");
    assert!(
        stdout(&output).is_empty(),
        "usage errors must not use stdout"
    );

    let text = stderr(&output);
    assert!(text.contains("error:"), "no error marker: {text}");
    assert!(text.contains("--help"), "no pointer to the help: {text}");
}

/// The help has to say what an argument-less call does, because the CLI
/// conventions leave that choice to the project rather than fixing it.
#[test]
fn the_help_states_what_an_argument_less_call_does() {
    let text = stdout(&run(&["--help"]));
    assert!(
        text.contains("Called without arguments"),
        "the help does not describe the default behaviour: {text}",
    );
}
