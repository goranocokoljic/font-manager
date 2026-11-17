//! E54.2 (ADR 0025): the app reads the text another app copied, on every CI
//! runner (Linux under `xvfb-run`). This process puts text on the system
//! clipboard the way another app's Copy does
//! ([`fm_platform::clipboard::probe`]); a child process of this test binary
//! reads it with [`fm_platform::clipboard::read_text`], the call behind the
//! text fields' Paste item.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // a test

use std::process::{Command, Output};

use fm_platform::clipboard::{probe, read_text};

const CHILD_TEST: &str = "child_reads_the_clipboard";

#[test]
#[ignore = "run as a child process by the test below"]
fn child_reads_the_clipboard() {
    let text = read_text().expect("the clipboard is readable");
    // On one line, so the parent finds it among the test harness output.
    println!("TEXT {}", text.replace('\n', "\\n"));
}

#[test]
fn a_second_process_reads_the_text_another_app_copied() {
    // Non-ASCII, a supplementary-plane character and a line break: the
    // UTF-16 and UTF-8 conversions on each OS keep every one.
    let copied = "Fönts — 𝔸 Ж\nline two";
    let child: Output = probe::offer(copied, || {
        Command::new(std::env::current_exe().expect("test binary"))
            .args([CHILD_TEST, "--exact", "--ignored", "--nocapture"])
            .output()
            .expect("the child runs")
    })
    .expect("the text is on the clipboard");
    let stdout = String::from_utf8_lossy(&child.stdout);
    assert!(
        child.status.success() && stdout.contains("1 passed"),
        "{stdout}{}",
        String::from_utf8_lossy(&child.stderr)
    );
    let read = stdout
        .lines()
        .find_map(|l| l.strip_prefix("TEXT "))
        .expect("the child printed the text");
    assert_eq!(read, copied.replace('\n', "\\n"));
}
