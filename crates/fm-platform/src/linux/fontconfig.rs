//! The fontconfig command-line tools the Linux activator drives: `fc-cache`
//! to refresh a font folder's cache, `fc-list` to find fonts by PostScript name.
//!
//! The tools are optional: a new process sees a link without the cache
//! (ADR 0008), so a missing `fc-cache` only makes other apps start slower.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::fs::path_from_bytes;

/// What the activator asks of fontconfig: the real tools on Linux, a fake in tests.
pub(crate) trait Fontconfig: Send + Sync + fmt::Debug {
    /// Refreshes the cache of `dir` without waiting for it; `force` rescans
    /// every file, which a removed link target needs (ADR 0008).
    fn refresh(&self, dir: &Path, force: bool);
    /// Every font fontconfig offers whose PostScript name is in `names`, with its file.
    fn fonts_named(&self, names: &HashSet<&str>) -> Result<Vec<(String, PathBuf)>, String>;
}

/// The `fc-cache` and `fc-list` programs on the `PATH`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FcTools;

impl Fontconfig for FcTools {
    fn refresh(&self, dir: &Path, force: bool) {
        let mut cmd = Command::new("fc-cache");
        if force {
            cmd.arg("-f");
        }
        let child = cmd
            .arg(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        // No `fc-cache`: the links work without the cache. The thread reaps the
        // child so it never lingers as a zombie; the caller does not wait.
        if let Ok(mut child) = child {
            std::thread::spawn(move || child.wait());
        }
    }

    fn fonts_named(&self, names: &HashSet<&str>) -> Result<Vec<(String, PathBuf)>, String> {
        let out = Command::new("fc-list")
            .args(["--format", "%{postscriptname}\t%{file}\n"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|e| format!("fc-list: {e}"))?;
        if !out.status.success() {
            return Err(format!("fc-list: {}", out.status));
        }
        Ok(parse_list(&out.stdout, names))
    }
}

/// The `name\tfile` lines of `fc-list` whose name is in `names`. A line
/// without a tab, or a name that is not UTF-8, is skipped; the file is kept
/// as bytes, so a path that is not UTF-8 survives.
pub(crate) fn parse_list(stdout: &[u8], names: &HashSet<&str>) -> Vec<(String, PathBuf)> {
    stdout
        .split(|b| *b == b'\n')
        .filter_map(|line| {
            let tab = line.iter().position(|b| *b == b'\t')?;
            let name = std::str::from_utf8(&line[..tab]).ok()?;
            let file = &line[tab + 1..];
            if !names.contains(name) || file.is_empty() {
                return None;
            }
            Some((name.to_owned(), path_from_bytes(file)?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_lines_are_filtered_by_name_and_keep_the_file() {
        let names: HashSet<&str> = ["Brand-Bold", "Other"].into_iter().collect();
        let mut stdout =
            b"Brand-Bold\t/usr/share/fonts/b.otf\nNope\t/x.ttf\nno tab\n\t/empty.ttf\nOther\t\n"
                .to_vec();
        stdout.extend_from_slice(b"Other\t/home/ada/\xff.ttf\n");
        let found = parse_list(&stdout, &names);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].0, "Brand-Bold");
        assert_eq!(found[0].1, PathBuf::from("/usr/share/fonts/b.otf"));
        assert_eq!(found[1].0, "Other");
        assert_eq!(
            found[1].1,
            path_from_bytes(b"/home/ada/\xff.ttf").expect("a path")
        );
    }
}
