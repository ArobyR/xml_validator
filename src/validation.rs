//! Running xmllint off the UI thread.
//!
//! Validation runs on a background thread with a time limit, so the window never
//! freezes and a hung process can be killed without closing the app.

use crate::classification::{classify, Detail, Phase, ValidationResult};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub enum RunOutcome {
    Completed {
        exit_code: Option<i32>,
        stderr: String,
        duration: Duration,
    },
    TimedOut {
        duration: Duration,
    },
    FailedToStart {
        error: String,
    },
}

/// Looks for xmllint on the PATH and in common locations off it (Homebrew on
/// macOS installs libxml2 as keg-only).
pub fn detect_xmllint() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![PathBuf::from("xmllint")];
    for path in [
        "/opt/homebrew/bin/xmllint",
        "/usr/local/bin/xmllint",
        "/usr/bin/xmllint",
    ] {
        candidates.push(PathBuf::from(path));
    }

    for candidate in &candidates {
        let mut cmd = Command::new(candidate);
        cmd.arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Ok(output) = cmd.output() {
            if output.status.success() {
                return Some(candidate.clone());
            }
        }
    }
    None
}

pub fn run(xmllint: &Path, xsd: &Path, xml: &Path, limit: Duration) -> RunOutcome {
    let start = Instant::now();
    let mut cmd = Command::new(xmllint);
    cmd.arg("--schema")
        .arg(xsd)
        .arg(xml)
        .arg("--noout")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            return RunOutcome::FailedToStart {
                error: e.to_string(),
            }
        }
    };

    let stderr = child.stderr.take().expect("stderr is piped");
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let mut stderr = stderr;
        let _ = stderr.read_to_string(&mut text);
        text
    });

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stderr = reader.join().unwrap_or_default();
                return RunOutcome::Completed {
                    exit_code: status.code(),
                    stderr,
                    duration: start.elapsed(),
                };
            }
            Ok(None) => {
                if start.elapsed() >= limit {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    return RunOutcome::TimedOut {
                        duration: start.elapsed(),
                    };
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return RunOutcome::FailedToStart {
                    error: e.to_string(),
                };
            }
        }
    }
}

pub fn run_and_classify(
    xmllint: &Path,
    xsd: &Path,
    xml: &Path,
    limit: Duration,
) -> ValidationResult {
    match run(xmllint, xsd, xml, limit) {
        RunOutcome::Completed {
            exit_code,
            stderr,
            duration,
        } => {
            let success = exit_code == Some(0);
            let phase = if success {
                Phase::SchemaViolation
            } else {
                classify(exit_code, &stderr)
            };
            ValidationResult {
                phase,
                success,
                stderr,
                exit_code,
                duration,
                detail: None,
            }
        }
        RunOutcome::TimedOut { duration } => ValidationResult {
            phase: Phase::Environment,
            success: false,
            stderr: String::new(),
            exit_code: None,
            duration,
            detail: Some(Detail::Timeout {
                seconds: limit.as_secs(),
            }),
        },
        RunOutcome::FailedToStart { error } => ValidationResult {
            phase: Phase::Environment,
            success: false,
            stderr: String::new(),
            exit_code: None,
            duration: Duration::ZERO,
            detail: Some(Detail::Spawn { error }),
        },
    }
}
