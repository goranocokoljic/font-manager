# 0026 — First-run setup

Status: Accepted (2026-11-18, E55.2)

## Context

A new user starts with an empty library and three decisions that live in three
places: which folders hold the fonts (one button in the empty state), whether
to use the online catalogs (off, in the sidebar), and where catalog downloads
go (asked in the middle of the first Install). The automatic update check is
off by default and found only under Settings → About. E55 adds a first-run
setup that asks these questions once, in order, and leaves out every question
that already has an answer.

The decisions below were made by Goran on 18 Nov 2026.

## Decision

**At first launch, not in the installer.** The setup runs in the app the first
time it starts (and at a later start while a step is still open). It is not
part of an installer:

- the macOS `.dmg` and the Linux AppImage have no installer to ask in: the
  user drags the app or runs the file;
- rule 5 (parity) wants the same setup on all three systems, so the Windows
  installer does not get questions the other two cannot ask;
- the app already has the folder picker, the catalogs and the settings that
  the answers go into; an installer would need a second copy of each.

**The steps, in the order the setup asks them, and when each is open.**
`fm_core::setup::open_steps` decides; `setup_status` in `SettingsService`
answers with the open steps, from the index's sources, the catalogs' on/off
file, `config.json` and whether this copy updates itself. It makes no network
request and does not scan.

| Step | Open while |
|------|------------|
| Font folders | the library has no watched folder and no library folder (the OS font folders do not count), and the step was never answered or skipped |
| Catalogs | no catalog is on, and the step was never answered or skipped |
| Download folder | a catalog is on and `download_folder` is not set |
| Updates | this copy updates itself (`UpdateSupport.offered`), `check_updates_automatically` is off, and the step was never answered or skipped |

The catalogs are asked about before their folder. The download folder has no
answer of its own: it closes when the user chooses the folder
(`update_settings` with `download_folder`), and `answer_setup_step` refuses it,
because a catalog install needs it to be one click.

**Answers are kept in `config.json`.** `answer_setup_step` records an answered
or skipped step under `setupAnswers` (a key with a default, so a file from an
earlier version reads as "nothing answered"). A skipped step is not asked
again at the next start. Nothing is kept in `localStorage`. An install from an
earlier version is asked only what its folders, catalogs and settings leave
open; with a watched folder, a catalog on, a download folder and the automatic
check on, it is asked nothing.

**Several font folders.** The font folders step lets the user add more than
one folder; each is a watched source, indexed in place (ADR 0004).

**Adobe Fonts is left out.** The app cannot download from Adobe Fonts, so the
setup does not offer it as a catalog.

**Where the rule lives.** The rule is in `fm-core`. The Rust `MockApi` and the
TypeScript mock each hold a copy (fm-api cannot depend on fm-core), and one
case table, `fixtures/setup/status-cases.json`, is run against all three, so
they give the same answer for the same state.

## Consequences

- A user who skips a step finds the same choice later where it always was
  (Sources, the catalogs in the sidebar, Settings → About); the setup does not
  come back for it.
- A catalog turned on later, outside the setup, opens the download folder step
  again until a folder is chosen.
- A copy whose updates come from a package manager or Flatpak is never asked
  about updates.
- The Catalogs and Download folder steps share one screen, "Online catalogs" (E55.5):
  an open Download folder step alone shows that screen. Choosing a download folder
  inside a system font folder is refused by `update_settings` (not only by
  `install_family`), so the setup and Settings say so before the first Install.
- A new setup step adds a `SetupStep` value, a row in the table above and
  cases in `fixtures/setup/status-cases.json`.
