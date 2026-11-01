//! The Settings… item of the menu bar and the tray (E5.12). Where it goes on
//! each OS comes from `fm_core::lifecycle::settings_menu`; this file only puts
//! it there, labels it in the UI's language (`shell_text`) and tells the
//! webview to open Settings. That message is a shell event (`shell:navigate`),
//! not an `fm-api` `Event`: it is about the window, not about fonts.

use fm_core::lifecycle::{self, SettingsMenu};
use fm_platform::Os;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Runtime};

use crate::shell_text;
use crate::tray::{self, MAIN_WINDOW};

/// The id of the menu bar's Settings… item.
pub const MENU_ID_SETTINGS: &str = "app:settings";
/// The id of the File menu the shell makes where the OS menu has none (Linux).
pub const FILE_SUBMENU_ID: &str = "app:file";
/// The shell event that asks the webview to go to a route; the payload is the
/// route's name. `onShellNavigate` in `src/api/client.ts` listens for it.
pub const SHELL_NAVIGATE: &str = "shell:navigate";
/// The route name of Settings in the UI's router.
pub const SETTINGS_ROUTE: &str = "settings";

/// The accelerator of the menu bar's Settings… item on `os`: Ctrl+, on
/// Windows and Linux, ⌘, on a Mac. Tauri has no getter for an item's
/// accelerator, so the tests check this.
pub fn settings_accelerator(os: Os) -> &'static str {
    match os {
        Os::Windows | Os::MacOs | Os::Linux => lifecycle::SETTINGS_ACCELERATOR,
    }
}

/// Adds Settings… to `menu` where `os` keeps it: above Quit in the app menu on
/// a Mac, at the top of the File menu elsewhere (made when the menu has none).
pub fn add_settings_item<R: Runtime>(
    app: &AppHandle<R>,
    menu: &Menu<R>,
    os: Os,
    locale: &str,
) -> tauri::Result<()> {
    let item = MenuItem::with_id(
        app,
        MENU_ID_SETTINGS,
        shell_text::text(locale, "shell.settings"),
        true,
        Some(settings_accelerator(os)),
    )?;
    match lifecycle::settings_menu(os) {
        SettingsMenu::AppMenuAboveQuit => {
            let Some(app_menu) = menu.items()?.first().and_then(|i| i.as_submenu().cloned()) else {
                return menu.prepend(&item);
            };
            let quit = app_menu.items()?.len().saturating_sub(1);
            app_menu.insert(&item, quit)?;
            app_menu.insert(&PredefinedMenuItem::separator(app)?, quit + 1)?;
        }
        SettingsMenu::FileMenu => {
            let file = match file_menu(menu)? {
                Some(file) => file,
                None => {
                    let file = Submenu::with_id(app, FILE_SUBMENU_ID, "File", true)?;
                    menu.prepend(&file)?;
                    file
                }
            };
            let others = !file.items()?.is_empty();
            file.prepend(&item)?;
            if others {
                file.insert(&PredefinedMenuItem::separator(app)?, 1)?;
            }
            file.set_text(shell_text::text(locale, "shell.fileMenu"))?;
        }
    }
    Ok(())
}

/// The OS menu's File menu: Tauri's default menu has one, untagged and first,
/// on Windows; the one the shell made on Linux.
fn file_menu<R: Runtime>(menu: &Menu<R>) -> tauri::Result<Option<Submenu<R>>> {
    for item in menu.items()? {
        if let Some(submenu) = item.as_submenu()
            && (submenu.id().as_ref() == FILE_SUBMENU_ID || submenu.text()? == "File")
        {
            return Ok(Some(submenu.clone()));
        }
    }
    Ok(None)
}

/// The submenu of `menu` that holds Settings….
pub fn settings_submenu<R: Runtime>(menu: &Menu<R>) -> Option<Submenu<R>> {
    menu.items().ok()?.into_iter().find_map(|i| {
        let submenu = i.as_submenu()?;
        submenu.get(MENU_ID_SETTINGS).map(|_| submenu.clone())
    })
}

/// Shows the menu bar's Settings… item (and the File menu that holds it on
/// Windows and Linux) in `locale`.
pub fn relabel<R: Runtime>(app: &AppHandle<R>, locale: &str) {
    let Some(submenu) = app.menu().as_ref().and_then(settings_submenu) else {
        return;
    };
    if let Some(item) = submenu
        .get(MENU_ID_SETTINGS)
        .and_then(|i| i.as_menuitem().cloned())
        && let Err(e) = item.set_text(shell_text::text(locale, "shell.settings"))
    {
        tracing::warn!(error = %e, "the Settings menu item was not relabelled");
    }
    if lifecycle::settings_menu(Os::current()) == SettingsMenu::FileMenu
        && let Err(e) = submenu.set_text(shell_text::text(locale, "shell.fileMenu"))
    {
        tracing::warn!(error = %e, "the File menu was not relabelled");
    }
}

/// Shows and focuses the main window, then asks it to open Settings.
pub fn open_settings<R: Runtime>(app: &AppHandle<R>) {
    tray::show_main(app);
    if let Err(e) = app.emit_to(MAIN_WINDOW, SHELL_NAVIGATE, SETTINGS_ROUTE) {
        tracing::warn!(error = %e, "Settings could not be opened");
    }
}

/// The menu bar's events: Settings… opens Settings, a Help page opens its page.
pub fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    if event.id().as_ref() == MENU_ID_SETTINGS {
        open_settings(app);
    } else {
        crate::help::on_menu_event(app, event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_os_gives_settings_the_ctrl_comma_accelerator() {
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            assert_eq!(settings_accelerator(os), "CmdOrCtrl+,", "{os:?}");
        }
    }

    #[test]
    fn the_shell_event_names_the_settings_route() {
        assert_eq!(SHELL_NAVIGATE, "shell:navigate");
        assert_eq!(SETTINGS_ROUTE, "settings");
    }
}
