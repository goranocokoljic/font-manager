//! `fm completions` and `fm man` are generated from the clap command tree:
//! every subcommand `--help` knows has a completion and a man page.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use clap::CommandFactory;
use fm_api::ErrorCode;
use fm_cli::{Cli, Shell};

/// Every subcommand path in the tree (`["sets", "export"]`), without
/// clap's `help`.
fn subcommands() -> Vec<Vec<String>> {
    fn walk(command: &clap::Command, prefix: &[String], all: &mut Vec<Vec<String>>) {
        for sub in command.get_subcommands().filter(|s| s.get_name() != "help") {
            let mut path = prefix.to_vec();
            path.push(sub.get_name().to_owned());
            walk(sub, &path, all);
            all.push(path);
        }
    }
    let mut all = Vec::new();
    walk(&Cli::command(), &[], &mut all);
    all
}

/// A folder under the system temp folder, removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("fm-gen-{label}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_tree_holds_the_commands_the_pages_are_checked_against() {
    let names: Vec<String> = subcommands().into_iter().map(|p| p.join(" ")).collect();
    for expected in [
        "version",
        "scan",
        "search",
        "activate",
        "doctor",
        "sets",
        "sets export",
        "sets import",
        "preview",
        "completions",
        "man",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} in {names:?}"
        );
    }
}

#[test]
fn every_completion_script_is_non_empty_and_names_every_subcommand() {
    for shell in Shell::ALL {
        let script = fm_cli::completions_script(shell).expect("script").script;
        assert!(!script.trim().is_empty(), "{shell:?} script is empty");
        for path in subcommands() {
            let name = path.last().expect("a name");
            assert!(
                script.contains(name.as_str()),
                "{shell:?} script does not name {}",
                path.join(" ")
            );
        }
    }
}

#[test]
fn a_man_page_is_made_for_fm_and_every_subcommand_from_its_help() {
    let pages = fm_cli::man_pages().expect("pages");
    let names: Vec<&str> = pages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names.first(), Some(&"fm.1"));
    let root = Cli::command();
    for path in subcommands() {
        let name = format!("fm-{}.1", path.join("-"));
        let page = pages
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no {name} in {names:?}"));
        // The page says what `--help` says: the first line of its about.
        let mut command = &root;
        for part in &path {
            command = command.find_subcommand(part).expect("subcommand");
        }
        let about = command.get_about().expect("an about").to_string();
        let first = about
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
        // Roff escapes the apostrophe and the hyphen.
        let roff = String::from_utf8(page.roff.clone())
            .expect("text")
            .replace(r"\*(Aq", "'")
            .replace(r"\-", "-");
        assert!(roff.contains(&first), "{name} lacks {first:?}");
        assert!(roff.contains(concat!("fm ", env!("CARGO_PKG_VERSION"))));
    }
    assert_eq!(pages.len(), subcommands().len() + 1, "{names:?}");
    // fm.1 carries the exit codes `fm --help` lists.
    let fm = String::from_utf8(pages[0].roff.clone()).expect("text");
    assert!(fm.contains("Exit codes:"), "{fm}");
}

#[test]
fn man_makes_the_folder_and_refuses_to_replace_a_page_without_force() {
    let dir = Dir::new("man");
    let out = dir.0.join("nested").join("man1");
    let written = fm_cli::write_man_pages(&out, false).expect("written");
    assert_eq!(written.files.len(), subcommands().len() + 1);
    for file in &written.files {
        assert!(PathBuf::from(file).is_absolute(), "{file}");
        assert!(std::fs::metadata(file).expect("page").len() > 0, "{file}");
    }

    // Without --force: refused, and no page is touched.
    let page = out.join("fm-scan.1");
    std::fs::write(&page, b"the user's own page").expect("write");
    std::fs::remove_file(out.join("fm.1")).expect("remove");
    let refused = fm_cli::write_man_pages(&out, false).expect_err("refused");
    assert_eq!(refused.code, ErrorCode::InvalidRequest);
    assert!(refused.message.contains("--force"), "{}", refused.message);
    assert_eq!(std::fs::read(&page).expect("page"), b"the user's own page");
    assert!(
        !out.join("fm.1").exists(),
        "nothing written after a refusal"
    );

    // With --force: every page written again.
    fm_cli::write_man_pages(&out, true).expect("forced");
    assert_ne!(std::fs::read(&page).expect("page"), b"the user's own page");
    assert!(out.join("fm.1").exists());
}

#[test]
fn man_into_a_file_instead_of_a_folder_is_an_io_error() {
    let dir = Dir::new("file");
    std::fs::create_dir_all(&dir.0).expect("dir");
    let file = dir.0.join("not-a-folder");
    std::fs::write(&file, b"x").expect("write");
    let failed = fm_cli::write_man_pages(&file, false).expect_err("failed");
    assert_eq!(failed.code, ErrorCode::Io);
    assert_eq!(std::fs::read(&file).expect("file"), b"x");
}
