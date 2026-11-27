# 0028 — One order of languages, decided in the core

Status: Accepted (2026-11-27, E58.1)

## Context

`list_languages` answered the presets of `data/charsets` in file-name order, then file
order: Arabic, Persian, the two Chinese, Japanese, Korean, Russian, Ukrainian, Belarusian
and Bulgarian came first, English 18th. Every list of languages (the filter drawer, the
sample text menu, the family page, the rule editor, Duplicates) shows that order. The
user already keeps an ordered list of the languages they work in
(`Settings::my_languages`, E56.3, ADR 0027), but it ordered nothing.

## Decision

Decided by Goran, 27 Nov 2026:

1. **One order, in the core.** `fm_core::language_order::arrange` orders the answer of
   `list_languages` in three parts, and the app, the CLI and MCP get the same answer. No
   adapter and no component sorts languages again.
   1. *The user's languages* — `Settings::my_languages` in the user's order.
   2. *The most used* — the codes of `data/charsets/order.json` that are not in part 1,
      in the file's order.
   3. *The others* — every other preset, from A to Z by its English name (case-blind,
      then by code).
   No preset is listed twice; an id of `my_languages` without a preset is skipped.
2. **The data file.** `data/charsets/order.json` (`mostUsed`, schema
   `order.schema.json` next to it) is community-editable like the other data files. The
   first draft is en, es, fr, de, pt, it, ru, zh-Hans, ja, ar. A code that no preset has
   fails the tests and the load.
3. **The system's language leads while no language is set.** With `my_languages` empty,
   the presets of the system's language form part 1: the preset of the script the tag
   names (`sr-Latn`), else every preset of the language in file order (`sr` → Serbian
   Cyrillic, then Latin). Where the language has no preset, nothing leads. The system's
   language is the locale of the shell's menu strings (`set_shell_locale`); the shell
   hands it to the core at start and on every change
   (`CoreApi::set_system_language`). The CLI's direct link has no shell, so there
   nothing leads.
4. **Each preset says its part.** `LanguagePreset::group` (`mine`, `mostUsed`,
   `others`; `#[serde(default)]`) lets a list draw a line between the parts without
   knowing the rule. The system's language, while it leads, is in `mine`: it stands in
   for the user's languages.
5. **A change is an event.** `Event::LanguageOrderChanged` is sent when an update
   changes `my_languages` and when the shell's language changes; `list_languages` after
   it answers in the new order.

The Duplicates rules and guards read `my_languages` as before (ADR 0027); this order
does not change them.

## Consequences

- The lists of languages lead with what the user needs; E58.3 makes every list follow
  this answer, E58.2 edits `my_languages` under Settings › Languages.
- The Rust MockApi and the TypeScript mock copy the rule;
  `fixtures/language-order/cases.json` runs against all three implementations.
- A new language in `order.json` is a data change, reviewed like any other.
