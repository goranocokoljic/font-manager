//! Whether this copy of the app offers "Check for updates" (E18.2, ADR 0012),
//! the platform note that says why not, and the release server it asks
//! (E55.7). [`on_quit`]: the files of "Install updates when I quit" (E55.9).

pub mod on_quit;

use fm_api::update::UpdateSupport;
use fm_platform::Distribution;

/// What Settings shows for a copy installed as `distribution`: the update
/// section for the release installers and the AppImage; a platform note
/// naming the tool that installs updates for every other copy.
pub fn support(distribution: &Distribution) -> UpdateSupport {
    let reason = match distribution {
        Distribution::Installer | Distribution::AppImage => None,
        Distribution::Flatpak => Some(
            "Flatpak installs the updates of this copy (flatpak update, or your software \
             center), so Check for updates is not shown."
                .to_owned(),
        ),
        Distribution::SystemPackage => Some(
            "Your system's package manager installs the updates of this copy, so Check for \
             updates is not shown. The AppImage updates itself."
                .to_owned(),
        ),
        Distribution::PackageManager(tool) => Some(format!(
            "{tool} installs the updates of this copy, so Check for updates is not shown."
        )),
        Distribution::NeedsAdmin => Some(
            "This copy is in a folder your account cannot change (installed for all users), \
             and Font Fu never asks for administrator rights, so Check for updates is \
             not shown. Install new versions from the releases page; the per-user installer \
             (.exe on Windows) updates itself."
                .to_owned(),
        ),
    };
    UpdateSupport {
        offered: reason.is_none(),
        reason,
        release_host: None,
    }
}

/// The host of the release server `endpoint` names (`github.com` for
/// `https://github.com/…/latest.json`), lower case, without user or port;
/// `None` when `endpoint` is not an `http(s)` address with a host.
pub fn release_host(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    let (scheme, rest) = endpoint.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if let Some(v6) = host_port.strip_prefix('[') {
        v6.split_once(']').map(|(h, _)| h)?
    } else {
        host_port.split(':').next().unwrap_or_default()
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// [`support`] for `distribution`, with the host of the first endpoint the
/// app was built with (the updater asks them in order).
pub fn support_from<'a>(
    distribution: &Distribution,
    endpoints: impl IntoIterator<Item = &'a str>,
) -> UpdateSupport {
    UpdateSupport {
        release_host: endpoints.into_iter().find_map(release_host),
        ..support(distribution)
    }
}

/// The page of what is new in `version` (the "Updated to <version>" link,
/// E55.9), from the first endpoint the app was built with that is a GitHub
/// release address (`https://github.com/<owner>/<repo>/releases/…`): that
/// repository's release page of the tag `v<version>`. `None` for any other
/// release server: it names no notes page.
pub fn release_notes_url<'a>(
    endpoints: impl IntoIterator<Item = &'a str>,
    version: &str,
) -> Option<String> {
    let tag = version.trim().trim_start_matches('v');
    if tag.is_empty()
        || !tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-+".contains(c))
    {
        return None;
    }
    endpoints.into_iter().find_map(|endpoint| {
        let rest = endpoint.trim().strip_prefix("https://github.com/")?;
        let mut parts = rest.split('/');
        let (owner, repo) = (parts.next()?, parts.next()?);
        let releases = parts.next()?;
        let name_ok = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        };
        (releases == "releases" && name_ok(owner) && name_ok(repo))
            .then(|| format!("https://github.com/{owner}/{repo}/releases/tag/v{tag}"))
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_notes_link_is_the_github_release_page_of_the_version() {
        let shipped =
            "https://github.com/goranocokoljic/font-manager/releases/latest/download/latest.json";
        assert_eq!(
            super::release_notes_url([shipped], "1.5.0").as_deref(),
            Some("https://github.com/goranocokoljic/font-manager/releases/tag/v1.5.0")
        );
        assert_eq!(
            super::release_notes_url(["https://fonts.example/latest.json", shipped], "v1.5.0")
                .as_deref(),
            Some("https://github.com/goranocokoljic/font-manager/releases/tag/v1.5.0")
        );
        assert_eq!(
            super::release_notes_url(["https://fonts.example/latest.json"], "1.5.0"),
            None
        );
        assert_eq!(super::release_notes_url([shipped], "1.5.0/../x"), None);
        assert_eq!(super::release_notes_url([shipped], ""), None);
        assert_eq!(
            super::release_notes_url(["https://github.com/o/r/archive/x"], "1.5.0"),
            None
        );
    }

    use super::*;

    #[test]
    fn installers_and_the_appimage_offer_updates_without_a_note() {
        for d in [Distribution::Installer, Distribution::AppImage] {
            assert_eq!(
                support(&d),
                UpdateSupport {
                    offered: true,
                    reason: None,
                    release_host: None,
                }
            );
        }
    }

    #[test]
    fn other_copies_hide_updates_and_name_the_tool_that_installs_them() {
        let flatpak = support(&Distribution::Flatpak);
        assert!(!flatpak.offered);
        assert!(
            flatpak
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("Flatpak"))
        );
        let deb = support(&Distribution::SystemPackage);
        assert!(!deb.offered);
        assert!(
            deb.reason
                .as_deref()
                .is_some_and(|r| r.contains("package manager"))
        );
        let admin = support(&Distribution::NeedsAdmin);
        assert!(!admin.offered);
        assert!(
            admin
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("never asks for administrator rights"))
        );
        let brew = support(&Distribution::PackageManager("Homebrew".into()));
        assert!(!brew.offered);
        assert!(
            brew.reason
                .as_deref()
                .is_some_and(|r| r.starts_with("Homebrew "))
        );
    }

    #[test]
    fn the_release_host_is_read_from_the_endpoint() {
        assert_eq!(
            release_host(
                "https://github.com/goranocokoljic/font-manager/releases/latest/download/latest.json"
            )
            .as_deref(),
            Some("github.com")
        );
        assert_eq!(
            release_host("HTTPS://User:pw@Updates.Example.org:8443/x?y#z").as_deref(),
            Some("updates.example.org")
        );
        assert_eq!(
            release_host("https://[::1]:8080/latest.json").as_deref(),
            Some("::1")
        );
        assert_eq!(
            release_host("https://fontfu.app").as_deref(),
            Some("fontfu.app")
        );
    }

    #[test]
    fn an_endpoint_without_a_web_host_names_none() {
        for bad in [
            "",
            "github.com/latest.json",
            "file:///tmp/latest.json",
            "https://",
            "https:///x",
        ] {
            assert_eq!(release_host(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn support_names_the_first_endpoint_with_a_host() {
        let s = support_from(
            &Distribution::Installer,
            [
                "not a url",
                "https://github.com/a/b/latest.json",
                "https://fontfu.app/latest.json",
            ],
        );
        assert!(s.offered);
        assert_eq!(s.release_host.as_deref(), Some("github.com"));
        let managed = support_from(&Distribution::Flatpak, ["https://github.com/x"]);
        assert!(!managed.offered);
        assert_eq!(managed.release_host.as_deref(), Some("github.com"));
        assert_eq!(
            support_from(&Distribution::Installer, []).release_host,
            None
        );
    }
}
