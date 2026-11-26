//! The Settings… item of the menu bar and the tray (E5.12), and the menu
//! bar's Duplicates item (E57.10). Where Settings… goes on
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
/// The id of the menu bar's Duplicates item (E57.10).
pub const MENU_ID_DUPLICATES: &str = "app:duplicates";
/// The id of the app's own Quit item in the menu bar (E55.9).
pub const MENU_ID_QUIT: &str = "app:quit";
/// The id of the File menu the shell makes where the OS menu has none (Linux).
pub const FILE_SUBMENU_ID: &str = "app:file";
/// The shell event that asks the webview to go to a route; the payload is the
/// route's name. `onShellNavigate` in `src/api/client.ts` listens for it.
pub const SHELL_NAVIGATE: &str = "shell:navigate";
/// The route name of Settings in the UI's router.
pub const SETTINGS_ROUTE: &str = "settings";
/// The route name of the Duplicates module in the UI's router.
pub const DUPLICATES_ROUTE: &str = "duplicates";
/// The name that asks the webview to open the update dialog (E55.7); not a
/// route, the UI handles it before its router.
pub const UPDATE_DIALOG: &str = "update";

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

/// Adds Duplicates to `menu` at the top of the File menu on every OS, a
/// separator under it (E57.10). Call it after [`add_settings_item`]: on
/// Windows and Linux it goes above Settings…, in the File menu that holds it;
/// on a Mac, where Settings… is in the app menu, in the OS's File menu (made
/// after the app menu when there is none). Always enabled: with no copies the
/// module shows its own empty overview.
pub fn add_duplicates_item<R: Runtime>(
    app: &AppHandle<R>,
    menu: &Menu<R>,
    os: Os,
    locale: &str,
) -> tauri::Result<()> {
    let item = MenuItem::with_id(
        app,
        MENU_ID_DUPLICATES,
        shell_text::text(locale, "shell.duplicates"),
        true,
        None::<&str>,
    )?;
    // The File menu that holds Settings… may already show the locale's
    // title, so it is found by its item, not by "File".
    let file = match lifecycle::settings_menu(os) {
        SettingsMenu::FileMenu => settings_submenu(menu),
        SettingsMenu::AppMenuAboveQuit => file_menu(menu)?,
    };
    let file = match file {
        Some(file) => file,
        None => {
            let file = Submenu::with_id(
                app,
                FILE_SUBMENU_ID,
                shell_text::text(locale, "shell.fileMenu"),
                true,
            )?;
            let after_app_menu = usize::from(os == Os::MacOs && !menu.items()?.is_empty());
            menu.insert(&file, after_app_menu)?;
            file
        }
    };
    let others = !file.items()?.is_empty();
    file.prepend(&item)?;
    if others {
        file.insert(&PredefinedMenuItem::separator(app)?, 1)?;
    }
    Ok(())
}

/// The submenu of `menu` that holds Duplicates.
pub fn duplicates_submenu<R: Runtime>(menu: &Menu<R>) -> Option<Submenu<R>> {
    menu.items().ok()?.into_iter().find_map(|i| {
        let submenu = i.as_submenu()?;
        submenu.get(MENU_ID_DUPLICATES).map(|_| submenu.clone())
    })
}

/// Replaces the default menu's Quit with the app's own item (E55.9), so a
/// quit from the menu is known as the user's: on a Mac the predefined item
/// terminates the app the same way a logout or a shutdown does, and an
/// update kept for the quit must not install during those. The app menu's
/// last item on a Mac (⌘Q), the File menu's last item on Windows; Linux's
/// default menu has no Quit.
pub fn replace_quit<R: Runtime>(
    app: &AppHandle<R>,
    menu: &Menu<R>,
    os: Os,
    locale: &str,
) -> tauri::Result<()> {
    let holder = match os {
        Os::MacOs => menu.items()?.first().and_then(|i| i.as_submenu().cloned()),
        Os::Windows => file_menu(menu)?,
        Os::Linux => None,
    };
    let Some(holder) = holder else {
        return Ok(());
    };
    let items = holder.items()?;
    let Some(last) = items.last() else {
        return Ok(());
    };
    let is_quit = match last.as_predefined_menuitem() {
        Some(item) => is_quit_text(&item.text()?),
        None => false,
    };
    if !is_quit {
        return Ok(());
    }
    holder.remove(last)?;
    let accelerator = (os == Os::MacOs).then_some("CmdOrCtrl+Q");
    let quit = MenuItem::with_id(
        app,
        MENU_ID_QUIT,
        shell_text::text(locale, "shell.tray.quit"),
        true,
        accelerator,
    )?;
    holder.append(&quit)
}

/// Whether `text` is the predefined Quit item's: "Quit Font Fu" on a Mac,
/// "&Exit" on Windows, "&Quit" elsewhere.
pub fn is_quit_text(text: &str) -> bool {
    let text = text.replace('&', "");
    text.starts_with("Quit") || text == "Exit"
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

/// Shows the menu bar's Settings… and Duplicates items (and the File menu
/// that holds them on Windows and Linux) in `locale`.
pub fn relabel<R: Runtime>(app: &AppHandle<R>, locale: &str) {
    if let Some(item) = app
        .menu()
        .as_ref()
        .and_then(duplicates_submenu)
        .and_then(|s| s.get(MENU_ID_DUPLICATES))
        .and_then(|i| i.as_menuitem().cloned())
        && let Err(e) = item.set_text(shell_text::text(locale, "shell.duplicates"))
    {
        tracing::warn!(error = %e, "the Duplicates menu item was not relabelled");
    }
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
    if let Some(quit) = submenu
        .get(MENU_ID_QUIT)
        .and_then(|i| i.as_menuitem().cloned())
        && let Err(e) = quit.set_text(shell_text::text(locale, "shell.tray.quit"))
    {
        tracing::warn!(error = %e, "the Quit menu item was not relabelled");
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

/// Shows and focuses the main window (also when it is closed to the tray),
/// then asks it to open the Duplicates module.
pub fn open_duplicates<R: Runtime>(app: &AppHandle<R>) {
    tray::show_main(app);
    if let Err(e) = app.emit_to(MAIN_WINDOW, SHELL_NAVIGATE, DUPLICATES_ROUTE) {
        tracing::warn!(error = %e, "Duplicates could not be opened");
    }
}

/// Shows and focuses the main window, then asks it to open the update
/// dialog: the tray's "Update to <version>…", a user's click.
pub fn open_update<R: Runtime>(app: &AppHandle<R>) {
    tray::show_main(app);
    if let Err(e) = app.emit_to(MAIN_WINDOW, SHELL_NAVIGATE, UPDATE_DIALOG) {
        tracing::warn!(error = %e, "the update dialog could not be opened");
    }
}

/// The menu bar's events: Settings… opens Settings, Duplicates the Duplicates
/// module, a Help page opens its page.
pub fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    if event.id().as_ref() == MENU_ID_SETTINGS {
        open_settings(app);
    } else if event.id().as_ref() == MENU_ID_DUPLICATES {
        open_duplicates(app);
    } else if event.id().as_ref() == MENU_ID_QUIT {
        crate::quit_install::quit(app);
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
        assert_eq!(UPDATE_DIALOG, "update");
        assert_eq!(DUPLICATES_ROUTE, "duplicates");
    }
}
