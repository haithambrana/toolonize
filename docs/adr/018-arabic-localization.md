# ADR-018: Arabic interface and right-to-left layout

Date: 2026-11-27
Status: **Accepted**

## Decision

- **No dependency.** `src/i18n` has `t(key, vars)` with the English text as
  the key (an untranslated string still shows, in English), `{name}`
  placeholders, and `tk()` to mark text in module-level constants that is
  translated where it is shown (the language is not known when modules
  load).
- **Setting.** Settings › Appearance › Language: Follow system / English /
  العربية (`language` in settings, validated by the core). Arabic sets
  `<html lang="ar" dir="rtl">`; the app tree is remounted on a change (the
  terminals live in long-lived controllers and keep their content).
- **Terminals stay left-to-right.** Terminal containers carry `dir="ltr"`
  and the pane layout keeps LTR, because terminal programs and xterm.js
  assume LTR cells; xterm.js does not shape or reorder Arabic, so text the
  app itself writes into terminals (the session-memory divider) stays in
  English.
- **Catalog checks** (`src/i18n/i18n.test.ts`): every literal passed to
  `t()`/`tk()` in the app has an Arabic entry, there are no stale entries,
  and placeholders match. An app test renders the workspace in Arabic and
  checks the direction, Arabic labels, LTR terminals and no English in the
  main toolbar.
- Whole sentences with placeholders instead of fragments, so Arabic word
  order works.

## Coverage

Translated: sidebar, workspace screen (toolbar, menus, layout modes,
startup prompt, empty states, notes drawer, delete dialog), panes and
their states, terminal menu, sticky notes, terminal settings, session
memory, backup/restore, templates, Settings, Launchers screen, Activity
screen headings and filters, search, command palette chrome, toasts for
restore/backup/templates.

Still English (follow-up): the launcher review and add-member dialogs,
keyboard action names in the shortcut table, Activity action names,
command-palette entry names, and error messages that come from the Rust
core.
