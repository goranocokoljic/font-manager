//! `fm completions` and `fm man`: shell completion scripts and man pages
//! generated from the clap command tree ([`Cli`]), the same definitions
//! `--help` prints, so the three cannot drift. They describe `fm` itself,
//! so they run without the app or the index.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::CommandFactory;
use fm_api::{ApiError, ErrorCode};
use serde::Serialize;

use crate::Cli;

/// How to install each script, as `fm completions --help` prints it.
pub const COMPLETIONS_INSTALL: &str = "\
Install:
  bash        fm completions bash > ~/.local/share/bash-completion/completions/fm
  zsh         mkdir -p ~/.zfunc && fm completions zsh > ~/.zfunc/_fm
              then in ~/.zshrc: fpath=(~/.zfunc $fpath); autoload -Uz compinit; compinit
  fish        fm completions fish > ~/.config/fish/completions/fm.fish
  powershell  add this line to $PROFILE:
              fm completions powershell | Out-String | Invoke-Expression
Open a new shell after installing.";

/// The shells `fm completions` writes a script for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Shell {
    /// Bash.
    Bash,
    /// Zsh.
    Zsh,
    /// Fish.
    Fish,
    /// PowerShell (Windows PowerShell 5.1 and PowerShell 7).
    Powershell,
}

impl Shell {
    /// Every shell, in the order `fm completions --help` lists them.
    pub const ALL: [Self; 4] = [Self::Bash, Self::Zsh, Self::Fish, Self::Powershell];

    fn generator(self) -> clap_complete::Shell {
        match self {
            Self::Bash => clap_complete::Shell::Bash,
            Self::Zsh => clap_complete::Shell::Zsh,
            Self::Fish => clap_complete::Shell::Fish,
            Self::Powershell => clap_complete::Shell::PowerShell,
        }
    }
}

/// `fm completions <shell>`: the script; `--json` prints it as `script`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Completions {
    /// The shell the script is for.
    pub shell: Shell,
    /// The completion script.
    pub script: String,
}

/// The completion script for `shell`, from the clap command tree.
pub fn completions(shell: Shell) -> Result<Completions, ApiError> {
    let mut script = Vec::new();
    clap_complete::generate(shell.generator(), &mut Cli::command(), "fm", &mut script);
    let script = String::from_utf8(script).map_err(|e| {
        ApiError::new(
            ErrorCode::Internal,
            "The completion script is not valid text.",
        )
        .with_detail(e.to_string())
    })?;
    Ok(Completions { shell, script })
}

/// `fm man --out <dir>`: the man pages written.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManPages {
    /// The absolute paths, `fm.1` first.
    pub files: Vec<String>,
}

/// One man page: its file name (`fm-sets-export.1`) and its roff text.
#[derive(Debug, Clone)]
pub struct ManPage {
    /// The file name.
    pub name: String,
    /// The page.
    pub roff: Vec<u8>,
}

/// `fm.1` and one page per subcommand (nested ones too, named
/// `fm-sets-export.1`), from the clap command tree. Hidden commands and
/// clap's `help` have no page.
pub fn man_pages() -> Result<Vec<ManPage>, ApiError> {
    let mut root = Cli::command().disable_help_subcommand(true);
    // Building names each subcommand `fm-<name>` for its page.
    root.build();
    let mut pages = Vec::new();
    render(&root, &mut pages)?;
    Ok(pages)
}

fn render(command: &clap::Command, pages: &mut Vec<ManPage>) -> Result<(), ApiError> {
    // Every page names `fm` and its version as its source, not the subcommand.
    let man =
        clap_mangen::Man::new(command.clone()).source(concat!("fm ", env!("CARGO_PKG_VERSION")));
    let mut roff = Vec::new();
    man.render(&mut roff).map_err(|e| {
        ApiError::new(ErrorCode::Internal, "A man page could not be made.")
            .with_detail(e.to_string())
    })?;
    pages.push(ManPage {
        name: man.get_filename(),
        roff,
    });
    for sub in command.get_subcommands().filter(|s| !s.is_hide_set()) {
        render(sub, pages)?;
    }
    Ok(())
}

/// Writes [`man_pages`] to `dir` (made when it is not there). A page of
/// the same name is replaced only with `force`; without it nothing is
/// written when any page exists.
pub fn write_man_pages(dir: &Path, force: bool) -> Result<ManPages, ApiError> {
    let pages = man_pages()?;
    let paths: Vec<PathBuf> = pages.iter().map(|p| dir.join(&p.name)).collect();
    if !force && let Some(path) = paths.iter().find(|p| p.symlink_metadata().is_ok()) {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            format!(
                "{} already exists. Use --force to replace it.",
                path.display()
            ),
        )
        .with_detail(path.display().to_string()));
    }
    let failed = |path: &Path, e: std::io::Error| {
        ApiError::new(
            ErrorCode::Io,
            format!("The man pages could not be written to {}.", path.display()),
        )
        .with_detail(e.to_string())
    };
    std::fs::create_dir_all(dir).map_err(|e| failed(dir, e))?;
    let mut files = Vec::with_capacity(pages.len());
    for (page, path) in pages.iter().zip(&paths) {
        write_page(path, &page.roff, force).map_err(|e| failed(path, e))?;
        let absolute = std::path::absolute(path).map_err(|e| failed(path, e))?;
        files.push(absolute.display().to_string());
    }
    Ok(ManPages { files })
}

/// A new file, or over an existing one with `force`.
fn write_page(path: &Path, roff: &[u8], force: bool) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        // A file made since the check is not replaced either.
        options.create_new(true);
    }
    options.open(path)?.write_all(roff)
}

/// `fm completions`: the script as it is.
pub fn script(c: &Completions, out: &mut dyn Write) -> std::io::Result<()> {
    out.write_all(c.script.as_bytes())?;
    out.flush()
}

/// `fm man`: the files written, one per line.
pub fn written(pages: &ManPages, out: &mut dyn Write) -> std::io::Result<()> {
    for file in &pages.files {
        writeln!(out, "{file}")?;
    }
    Ok(())
}
