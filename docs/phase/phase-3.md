# Phase 3: Claude Code Plugin Integration

**Goal:** Provide a `initialize-flutter` plugin command with Flutter-specific rules and slash commands.

## Tasks

- [x] 3.1 Create `plugin/commands/initialize-flutter.md` — document Flutter project setup steps, recommended `CLAUDE.md` snippets, and common `ast-index` invocations for Flutter codebases
- [x] 3.2 Add Flutter-specific search rules to the plugin command (e.g. widget class detection, provider/bloc pattern hints)
- [x] 3.3 Add Flutter-specific slash command examples (e.g. `ast-index search --kind Class --lang dart`)
- [x] 3.4 Verify `plugin/SKILL.md` (or equivalent index) references the new `initialize-flutter` command

## Acceptance Criteria

**Test:** The `initialize-flutter` command file is present and loadable; content covers Flutter project conventions.

## Dependencies

- Phase 2 complete

## Implementation Notes

Key files to create/modify:
- `plugin/commands/initialize-flutter.md` — new file with Flutter setup guide
- `plugin/SKILL.md` — add reference to the new command
