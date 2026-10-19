//! The Help menu and the help window. The user docs (`docs/user/`) are built by
//! `scripts/docs/build-site.mjs` into `public/help/`, which Vite bundles with the
//! app, so the pages open offline in an app window (CLAUDE.md rule 7). The help
//! window has no capability: the pages are static and need no IPC.

use fm_api::{ApiError, ApiResult, ErrorCode};
use tauri::menu::{HELP_SUBMENU_ID, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

/// One page of the user docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpPage {
    /// The file name without extension: `docs/user/<slug>.md`, `help/<slug>.html`.
    pub slug: &'static str,
    /// The menu item's text.
    pub title: &'static str,
}

/// The pages the Help menu lists, in menu order. `PAGES` in
/// `scripts/docs/build-site.mjs` and `HELP_PAGES` in `src/help.ts` list the
/// same slugs; `scripts/docs/site.test.mjs` checks it.
pub const HELP_PAGES: &[HelpPage] = &[
    HelpPage {
        slug: "getting-started",
        title: "Getting started",
    },
    HelpPage {
        slug: "sources",
        title: "Sources",
    },
    HelpPage {
        slug: "activation",
        title: "Activation",
    },
    HelpPage {
        slug: "keyboard-shortcuts",
        title: "Keyboard shortcuts",
    },
];

/// The label of the one help window; opening another page reuses it.
pub const HELP_WINDOW: &str = "help";

const MENU_ID_PREFIX: &str = "help:";

/// The page with this slug.
pub fn page(slug: &str) -> Option<&'static HelpPage> {
    HELP_PAGES.iter().find(|p| p.slug == slug)
}

/// The id of a page's Help menu item.
pub fn menu_id(page: &HelpPage) -> String {
    format!("{MENU_ID_PREFIX}{}", page.slug)
}

/// The page a Help menu item opens; `None` for every other menu item.
pub fn page_for_menu_id(id: &str) -> Option<&'static HelpPage> {
    id.strip_prefix(MENU_ID_PREFIX).and_then(page)
}

/// The page's path inside the app's bundled frontend.
pub fn page_path(page: &HelpPage) -> String {
    format!("help/{}.html", page.slug)
}

/// Opens a page in the help window: creates the window, or moves the open one
/// to the page and brings it to the front.
pub fn open<R: Runtime>(app: &AppHandle<R>, slug: &str) -> ApiResult<()> {
    let page = page(slug).ok_or_else(|| {
        ApiError::new(ErrorCode::NotFound, "This help page does not exist.")
            .with_detail(format!("unknown help page {slug:?}"))
    })?;
    let failed = |e: tauri::Error| {
        ApiError::new(ErrorCode::Internal, "The help page could not be opened.")
            .with_detail(e.to_string())
    };
    if let Some(window) = app.get_webview_window(HELP_WINDOW) {
        // The help window only ever shows the bundled pages (the site links to
        // nothing else), so the page sits next to the current one.
        let url = window
            .url()
            .map_err(failed)?
            .join(&format!("{}.html", page.slug))
            .map_err(|e| {
                ApiError::new(ErrorCode::Internal, "The help page could not be opened.")
                    .with_detail(e.to_string())
            })?;
        window.navigate(url).map_err(failed)?;
        window.set_focus().map_err(failed)?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, HELP_WINDOW, WebviewUrl::App(page_path(page).into()))
        .title("Font Manager Help")
        .inner_size(960.0, 720.0)
        .min_inner_size(480.0, 400.0)
        .build()
        .map_err(failed)?;
    Ok(())
}

/// The app menu: Tauri's default menus on every OS, with the help pages at the
/// top of the Help menu.
pub fn menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;
    if let Some(help) = menu
        .get(HELP_SUBMENU_ID)
        .and_then(|i| i.as_submenu().cloned())
    {
        for (position, page) in HELP_PAGES.iter().enumerate() {
            let item = MenuItem::with_id(app, menu_id(page), page.title, true, None::<&str>)?;
            help.insert(&item, position)?;
        }
        // The default Help menu holds About on Windows and Linux.
        if help.items()?.len() > HELP_PAGES.len() {
            help.insert(&PredefinedMenuItem::separator(app)?, HELP_PAGES.len())?;
        }
    }
    Ok(menu)
}

/// Opens the page of a Help menu item; ignores every other menu event.
pub fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    if let Some(page) = page_for_menu_id(event.id().as_ref())
        && let Err(e) = open(app, page.slug)
    {
        tracing::warn!(error = ?e, page = page.slug, "could not open a help page");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn every_menu_id_opens_its_own_page() {
        for page in HELP_PAGES {
            assert_eq!(page_for_menu_id(&menu_id(page)), Some(page));
        }
    }

    #[test]
    fn other_menu_ids_open_no_page() {
        assert_eq!(page_for_menu_id("help:nope"), None);
        assert_eq!(page_for_menu_id("getting-started"), None);
        assert_eq!(page_for_menu_id(""), None);
    }

    #[test]
    fn the_menu_lists_the_four_required_pages() {
        let slugs: Vec<_> = HELP_PAGES.iter().map(|p| p.slug).collect();
        assert_eq!(
            slugs,
            [
                "getting-started",
                "sources",
                "activation",
                "keyboard-shortcuts"
            ]
        );
    }

    #[test]
    fn every_page_has_its_source_in_docs_user() {
        let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/user");
        for page in HELP_PAGES {
            let file = docs.join(format!("{}.md", page.slug));
            let text = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            assert!(
                text.starts_with(&format!("# {}\n", page.title))
                    || text.starts_with(&format!("# {}\r\n", page.title)),
                "{} should start with \"# {}\"",
                file.display(),
                page.title
            );
        }
    }

    #[test]
    fn pages_are_served_from_the_help_folder() {
        assert_eq!(page_path(&HELP_PAGES[1]), "help/sources.html");
    }
}
