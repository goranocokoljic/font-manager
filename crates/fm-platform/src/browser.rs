//! Opens a web page in the user's default browser (E22.6: the "Open Adobe
//! Fonts" link). Only `https` addresses are opened: the app never hands a
//! `file:`, `javascript:` or custom-scheme address to the OS. Each OS builds
//! its own command in `src/{windows,macos,linux}/open_url.rs`; the commands
//! are plain data, so their unit tests run on every CI runner without
//! launching a browser.

use std::ffi::OsString;
use std::process::{Command, Stdio};

use crate::Os;

/// The program the OS runs to open an address, and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCommand {
    /// The program (`xdg-open`, `/usr/bin/open`, `rundll32.exe`).
    pub program: OsString,
    /// Its arguments; the address is the last one.
    pub args: Vec<OsString>,
}

/// Why an address was not opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenUrlError {
    /// The address is not an `https://` address with a host, or holds
    /// whitespace, quotes or control characters.
    NotHttps(String),
    /// The OS command could not be started.
    Launch(String),
}

impl std::fmt::Display for OpenUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotHttps(url) => write!(f, "not an https address: {url:?}"),
            Self::Launch(detail) => write!(f, "could not start the browser: {detail}"),
        }
    }
}

impl std::error::Error for OpenUrlError {}

/// Checks that `url` is an `https://` address with a host and no character a
/// shell or the OS opener could read as something else.
pub fn check_https(url: &str) -> Result<(), OpenUrlError> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| OpenUrlError::NotHttps(url.to_owned()))?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.rsplit('@').next().unwrap_or_default();
    let safe = url
        .chars()
        .all(|c| c.is_ascii_graphic() && !matches!(c, '"' | '\'' | '`' | '\\' | '<' | '>'));
    if host.is_empty() || host.starts_with(':') || !safe {
        return Err(OpenUrlError::NotHttps(url.to_owned()));
    }
    Ok(())
}

/// The command that opens `url` on `os`; `url` must pass [`check_https`].
pub fn command_for(os: Os, url: &str) -> Result<OpenCommand, OpenUrlError> {
    check_https(url)?;
    Ok(match os {
        Os::Windows => crate::windows::open_url_command(url),
        Os::MacOs => crate::macos::open_url_command(url),
        Os::Linux => crate::linux::open_url_command(url),
    })
}

/// Opens an `https://` address in the default browser. Returns once the OS
/// opener started; a thread reaps it, so it never lingers as a zombie.
pub fn open_url(url: &str) -> Result<(), OpenUrlError> {
    let cmd = command_for(Os::current(), url)?;
    let mut child = Command::new(&cmd.program)
        .args(&cmd.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| OpenUrlError::Launch(format!("{}: {e}", cmd.program.to_string_lossy())))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADOBE: &str = "https://fonts.adobe.com";

    fn strings(cmd: &OpenCommand) -> (String, Vec<String>) {
        (
            cmd.program.to_string_lossy().into_owned(),
            cmd.args
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect(),
        )
    }

    #[test]
    fn https_addresses_pass() {
        for url in [
            ADOBE,
            "https://fonts.adobe.com/",
            "https://fonts.adobe.com/fonts?q=sans#top",
            "https://example.com:8443/a",
        ] {
            assert_eq!(check_https(url), Ok(()), "{url}");
        }
    }

    #[test]
    fn everything_but_https_is_refused() {
        for url in [
            "",
            "http://fonts.adobe.com",
            "HTTPS://fonts.adobe.com",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "ms-settings:fonts",
            "https://",
            "https:///path",
            "https://:443/",
            "https://user@/",
            "https://fonts.adobe.com/a b",
            "https://fonts.adobe.com/\"&calc.exe",
            "https://fonts.adobe.com/\n",
            "https://fonts.adobe.com/é",
            " https://fonts.adobe.com",
        ] {
            assert!(
                matches!(check_https(url), Err(OpenUrlError::NotHttps(_))),
                "{url:?} passed"
            );
            for os in [Os::Windows, Os::MacOs, Os::Linux] {
                assert!(command_for(os, url).is_err(), "{os:?} {url:?}");
            }
        }
    }

    #[test]
    fn windows_hands_the_address_to_the_url_protocol_handler() {
        let cmd = command_for(Os::Windows, ADOBE).unwrap();
        assert_eq!(
            strings(&cmd),
            (
                "rundll32.exe".into(),
                vec!["url.dll,FileProtocolHandler".into(), ADOBE.into()]
            )
        );
    }

    #[test]
    fn macos_runs_open() {
        let cmd = command_for(Os::MacOs, ADOBE).unwrap();
        assert_eq!(strings(&cmd), ("/usr/bin/open".into(), vec![ADOBE.into()]));
    }

    #[test]
    fn linux_runs_xdg_open() {
        let cmd = command_for(Os::Linux, ADOBE).unwrap();
        assert_eq!(strings(&cmd), ("xdg-open".into(), vec![ADOBE.into()]));
    }

    #[test]
    fn this_os_builds_its_own_command() {
        let expected = match Os::current() {
            Os::Windows => "rundll32.exe",
            Os::MacOs => "/usr/bin/open",
            Os::Linux => "xdg-open",
        };
        let cmd = command_for(Os::current(), ADOBE).unwrap();
        assert_eq!(cmd.program, OsString::from(expected));
        assert_eq!(cmd.args.last(), Some(&OsString::from(ADOBE)));
    }

    #[test]
    fn open_url_refuses_before_launching_anything() {
        assert!(matches!(
            open_url("file:///etc/passwd"),
            Err(OpenUrlError::NotHttps(_))
        ));
    }
}
