# Summary: CS-3-3 -- Flutter-Specific Plugin Command (Phase 3 Complete)

**Ticket:** CS-3-3
**Status:** IMPLEMENT_STEP_OK
**Date:** 2026-03-01

---

## What Was Done

Phase 3 (Claude Code Plugin Integration) adds the `initialize-flutter` plugin command to `plugin/commands/`. This ticket implements all four Phase 3 tasks in a single file delivery:

- Task 3.1: Created `plugin/commands/initialize-flutter.md` with the standard 5-step initialize structure
- Task 3.2: Added Flutter-specific search rules (widget detection, state management hints) as a textual guidelines section inside the rules content block
- Task 3.3: Added a 10-row Flutter/Dart-Specific Commands table inside the rules content block
- Task 3.4: Verified that `plugin/skills/ast-index/SKILL.md` and `plugin/.claude-plugin/plugin.json` already cover Dart/Flutter -- no changes were needed to those files

One file was created:

### `plugin/commands/initialize-flutter.md` (NEW)

The file follows the exact structure of the six existing initialize commands (`initialize-android.md`, `initialize-ios.md`, `initialize-web.md`, `initialize-ruby.md`, `initialize-rust.md`, `initialize-csharp.md`). Content breakdown:

**Frontmatter:**
```yaml
name: initialize-flutter
description: Initialize ast-index for Dart/Flutter project - configures .claude/settings.json, rules, and CLAUDE.md
```

**Step 1 -- Prerequisites:** `ast-index version` check with `brew tap`/`brew install` fallback. Identical to all other initialize commands.

**Step 2 -- settings.json:** Standard JSON block adding `ast-index` to `extraKnownMarketplaces`, `enabledPlugins`, and `permissions`. Identical to all other initialize commands.

**Step 3 -- Rules file (CRITICAL):** Creates `.claude/rules/ast-index.md` with the following sections:
- Mandatory Search Rules (identical across platforms): four rules (ALWAYS use first, NEVER duplicate, DO NOT grep for completeness, grep only for empty results / regex / string literals / comments)
- Why ast-index (identical): 17-69x faster than grep
- Command Reference table (3-column, Dart-adapted): `outline "widget.dart"` as the file outline example
- **Flutter-Specific Search Rules** (Task 3.2): six textual guidelines covering widget detection (`implementations "StatelessWidget"`, `implementations "StatefulWidget"`), state management (`implementations "Bloc"`, `implementations "ChangeNotifier"`), Dart constructs (`class "Mixin"`, `search "extension"`), and architecture detection (`hierarchy "Widget"`, `conventions`)
- **Flutter/Dart-Specific Commands** table (Task 3.3): 10-row, 2-column `| Task | Command |` table covering StatelessWidget, StatefulWidget, BLoC, ChangeNotifier, Cubit, mixins, extensions, widget hierarchy, Navigator usages, and project map
- Index Management (identical): rebuild, update, stats

**Step 4 -- Build Index:** `ast-index rebuild`. Identical to all other initialize commands.

**Step 5 -- Verify Setup:** `ast-index stats` + `ast-index search "Widget"`. Uses the Flutter-relevant term `"Widget"` rather than a platform-agnostic or Android-specific term (per PRD Scenario 6).

**Output section:** Standard completion message listing settings.json, rules file, and index stats.

**Flutter Project Detection section:** Optional detection commands for BLoC (`implementations "Bloc"`), Provider (`implementations "ChangeNotifier"`), Riverpod (`search "ConsumerWidget"`), and conventions (`conventions`). Follows the pattern of `initialize-ruby.md`'s "Rails Project Detection" section.

---

## Decisions Made

### All four Phase 3 tasks delivered in one ticket

The plan consolidated Tasks 3.1, 3.2, 3.3, and 3.4 into a single ticket rather than sequencing them across three separate tickets (as the original phase description implied). This eliminated the stated dependency chain (CS-3-1 -> CS-3-2 -> CS-3-3) and reduced coordination overhead. The resulting file contains all content in one pass.

### `ast-index search --kind Class --lang dart` not used

The `phase-3.md` description file gave `ast-index search --kind Class --lang dart` as an illustrative example. The actual `ast-index` CLI's `search` subcommand has no `--kind` or `--lang` flags. The plan resolved this by using `ast-index class "ClassName"` for class lookups and `ast-index symbol "SymbolName"` for other symbol kinds. All 15 commands appearing in the file were validated against `src/main.rs` before inclusion.

### `ast-index modules` subcommand does not exist

PRD Scenario 5 mentions `ast-index modules` as an expected example command. The actual subcommand is `ast-index module "pattern"` (singular, requires a pattern argument). The Flutter/Dart-Specific Commands table omits `module` and instead uses `ast-index map` for the "project overview" row, which provides module information alongside the project structure without requiring a pattern argument.

### Mixin search via `ast-index class "Mixin"`

Dart mixins are indexed with `SymbolKind::Interface` rather than a dedicated mixin kind. The `class` subcommand searches across all class-like kinds (Class, Interface, Trait). The guideline `ast-index class "Mixin"` therefore finds mixins by naming convention substring match -- it returns mixins whose name contains the string "Mixin". This is a known limitation documented in the PRD risks section and consistent with how other platforms use convention-based naming (e.g., Android `class "Controller"`).

### SKILL.md and plugin.json required no changes (Task 3.4)

`plugin/.claude-plugin/plugin.json` uses `"commands": "./commands"` for auto-discovery, so placing `initialize-flutter.md` in `plugin/commands/` is sufficient for the command to be registered. `plugin/skills/ast-index/SKILL.md` already references Dart/Flutter in its description field, supported projects table, and platform-specific commands section (pointing to `references/dart-commands.md`). No modifications to these files were needed.

### Pre-existing `--kind` flag bug in `initialize-web.md` not fixed

The existing `initialize-web.md` uses `ast-index search "use" --kind function`, which references the non-existent `--kind` flag on `search`. This is a pre-existing defect. The Flutter command does not replicate this mistake, but fixing the web command was explicitly declared out of scope for CS-3-3 to avoid unintended regressions.

---

## Files Created

| File | Action |
|------|--------|
| `plugin/commands/initialize-flutter.md` | Created (178 lines) |

---

## Files Verified (No Changes)

| File | Finding |
|------|---------|
| `plugin/skills/ast-index/SKILL.md` | Already references Dart/Flutter throughout |
| `plugin/.claude-plugin/plugin.json` | Uses `"commands": "./commands"` auto-discovery |
| `plugin/skills/ast-index/references/dart-commands.md` | Already documents Dart-specific capabilities |

---

## Flutter/Dart-Specific Commands Table (as delivered)

| Task | Command |
|------|---------|
| Find StatelessWidget implementations | `ast-index implementations "StatelessWidget"` |
| Find StatefulWidget implementations | `ast-index implementations "StatefulWidget"` |
| Find BLoC classes | `ast-index implementations "Bloc"` |
| Find ChangeNotifier classes | `ast-index implementations "ChangeNotifier"` |
| Find Cubit classes | `ast-index implementations "Cubit"` |
| Find mixins | `ast-index class "Mixin"` |
| Find extensions | `ast-index search "extension"` |
| Find widget hierarchy | `ast-index hierarchy "Widget"` |
| Find Navigator usages | `ast-index usages "Navigator"` |
| Project overview | `ast-index map` |

---

## Scope Boundary

This ticket covers Phase 3 (Claude Code Plugin Integration) only. It does not modify any Rust source files, parsers, or the SQLite schema. The `initialize-flutter.md` file is a documentation-only deliverable that configures Claude Code's behavior in Flutter projects.
