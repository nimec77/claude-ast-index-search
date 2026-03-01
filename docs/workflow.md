# Feature Development Workflow

This document describes the end-to-end AI-driven feature development workflow used in the `ast-index` project. For coding conventions, see [`docs/conventions_example.md`](conventions_example.md).

## Overview

Features progress through a series of quality gates, each producing specific artifacts. The workflow can be orchestrated in three modes:

- **`/feature-development`** — full lifecycle with user review checkpoints
- **`/dev-cycle`** — fully automatic, no user checkpoints, commits and tags a release
- **`/phase-loop`** — continuous phase iteration using `/quick-implement` for rapid delivery

## Workflow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Feature Development Flow                             │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
  │ ANALYSIS │───▶│ RESEARCH │───▶│   PLAN   │───▶│ TASKLIST │
  │ /analysis│    │ /research│    │  /plan   │    │/tasklist │
  └──────────┘    └──────────┘    └──────────┘    └──────────┘
       │               │               │               │
       ▼               ▼               ▼               ▼
   PRD file       Research doc    Plan document    Tasklist
                                                       │
                                                       ▼
  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
  │  DOCS    │◀───│    QA    │◀───│  REVIEW  │◀───│IMPLEMENT │
  │/docs-upd │    │   /qa    │    │/run-revw │    │/implement│
  └──────────┘    └──────────┘    └──────────┘    └──────────┘
       │               │               │               │
       ▼               ▼               ▼               ▼
   Changelog       QA report     Review notes    Working code
   & summary
```

## Quality Gates

Each gate must pass before proceeding to the next. Gates are executed sequentially.

| Gate | Condition to Pass | Artifact Produced |
|------|-------------------|-------------------|
| **PRD_READY** | PRD file exists | `docs/prd/<ticket>.prd.md` |
| **PLAN_APPROVED** | Plan file exists with `Status: PLAN_APPROVED` | `docs/plan/<ticket>.md` |
| **TASKLIST_READY** | Tasklist file exists with `Status: TASKLIST_READY` | `docs/tasklist/<ticket>.md` |
| **IMPLEMENT_STEP_OK** | All tasks marked `[x]` in tasklist | Modified source files |
| **REVIEW_OK** | Code review completed | Review notes |
| **RELEASE_READY** | QA report generated | `reports/qa/<ticket>.md` |
| **DOCS_UPDATED** | Documentation updated | `CHANGELOG.md`, summary file |

## Artifacts Directory Structure

```
docs/
├── .active_ticket            # Current ticket ID (e.g., CS-1-1)
├── prd/
│   └── <ticket>.prd.md       # Product Requirements Document
├── research/
│   └── <ticket>.md           # Technical research findings
├── plan/
│   └── <ticket>.md           # Architecture and implementation plan
├── tasklist/
│   └── <ticket>.md           # Breakdown of tasks with checkboxes
├── tasklist.md               # Global phase-based tasklist (used by /phase-loop)
├── adr/
│   └── <ticket>.md           # Architecture Decision Records (if alternatives exist)
├── phase/
│   └── phase-N.md            # Individual phase files for /phase-loop and /quick-implement
├── summaries/
│   └── <ticket>-summary.md   # Per-ticket decision summaries (created by /docs-update)
├── templates/
│   └── tasklist.example.md   # Template for new tasklists
└── releases/
    └── <release>.md          # Release bundles (R-prefixed IDs)

reports/
└── qa/
    └── <ticket>.md           # QA plan and verdict
```

## Slash Commands Reference

### Orchestration

| Command | Description | Usage |
|---------|-------------|-------|
| `/feature-development` | End-to-end workflow with user checkpoints at PRD, plan, and implementation review. Commits and tags a minor release on completion. | `/feature-development <ticket-id> [description-file]` |
| `/dev-cycle` | Fully automatic lifecycle — no user checkpoints. Runs all gates, commits, and tags a release. | `/dev-cycle <ticket-id> [description-file]` |
| `/phase-loop` | Continuous phase loop: syncs `docs/tasklist.md` status, runs `/quick-implement` on the next incomplete phase, commits, and repeats. Up to 10 iterations per run. | `/phase-loop [--no-commit]` |

### Phase Commands

Run these individually for fine-grained control:

#### `/analysis`
**Purpose:** Initialize a feature ticket and create the PRD.

**What it does:**
1. Sets `docs/.active_ticket` to the ticket ID
2. Creates `docs/prd/<ticket>.prd.md` from template
3. Populates sections: goals, user stories, scenarios, metrics, constraints, risks, open questions
4. Sets status to `DRAFT` or `PRD_READY`

**Usage:** `/analysis <ticket-id> [description-file]`

**Example:** `/analysis CS-1-2 "Flutter Parser" docs/phase/phase-2.md`

---

#### `/research`
**Purpose:** Gather technical context before planning.

**What it does:**
1. Reads the PRD and asks clarifying questions via interactive prompts
2. Scans codebase for related entities, patterns, and dependencies
3. Documents findings in `docs/research/<ticket>.md`
4. Does NOT modify code

**Usage:** `/research <ticket-id>`

**Example:** `/research CS-1-1`

---

#### `/plan`
**Purpose:** Create architecture and implementation plan.

**What it does:**
1. Reads PRD, research doc, and conventions
2. Creates `docs/plan/<ticket>.md` with: components, API contract, data flows, NFRs, risks
3. Creates `docs/adr/<ticket>.md` if architectural alternatives exist
4. Sets `Status: PLAN_APPROVED` when complete

**Usage:** `/plan <ticket-id>`

**Example:** `/plan CS-1-1`

---

#### `/tasklist`
**Purpose:** Break down the plan into actionable tasks.

**What it does:**
1. Requires `PLAN_APPROVED` status
2. Creates `docs/tasklist/<ticket>.md` with checkbox tasks
3. Each task has 1-2 acceptance criteria
4. Sets `Status: TASKLIST_READY` when complete

**Usage:** `/tasklist <ticket-id>`

**Example:** `/tasklist CS-1-1`

---

#### `/implement-orchestrated`
**Purpose:** Implement tasks with code/test separation and automated verification.

**What it does:**
1. Finds first incomplete task `- [ ]` in tasklist
2. Creates git savepoint for rollback
3. Implements production code
4. Optionally writes tests
5. Runs verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`
6. Refines up to 3 times if verification fails
7. Marks task complete `- [x]` on success
8. Rolls back on failure after max iterations

**Usage:** `/implement-orchestrated <ticket-id>`

**Example:** `/implement-orchestrated CS-1-1`

---

#### `/quick-implement`
**Purpose:** Implement all unchecked tasks in a phase file with automated coding and review. Used by `/phase-loop`.

**What it does:**
1. Reads the specified phase file for `- [ ]` tasks
2. For each task: invokes a `coder` subagent (writes code + tests), then a `reviewer` subagent (runs `cargo fmt`, `cargo clippy`, `cargo test`)
3. Refines up to 3 times on build failure, 2 times on review failure
4. Marks each task `- [x]` on success
5. Processes all tasks without confirmation prompts

**Usage:** `/quick-implement <phase-file-path>`

**Example:** `/quick-implement docs/phase/phase-1.md`

---

#### `/run-reviewer`
**Purpose:** Review code changes against requirements.

**What it does:**
1. Reads PRD, plan, tasklist, and conventions
2. Analyzes diff for ticket-related changes
3. Generates review with: blocking issues, important recommendations, cosmetic notes
4. Suggests additional tasks if gaps found

**Usage:** `/run-reviewer <ticket-id>`

**Example:** `/run-reviewer CS-1-1`

---

#### `/qa`
**Purpose:** Generate QA plan and verdict.

**What it does:**
1. For releases (R-prefixed), extracts all related tickets
2. Reads all artifacts for the ticket(s)
3. Creates `reports/qa/<ticket>.md` with:
   - Positive scenarios
   - Negative and edge cases
   - Automated vs manual test division
   - Risk zones
   - Verdict: release / with reservations / do not release

**Usage:** `/qa <ticket-id>` or `/qa <release-id>`

**Example:** `/qa CS-1-1` or `/qa R-3.22`

---

#### `/docs-update`
**Purpose:** Update documentation after implementation.

**What it does:**
1. Reads all artifacts and code diff
2. Creates `docs/summaries/<ticket>-summary.md` documenting decisions
3. Adds entry to `CHANGELOG.md`

**Usage:** `/docs-update <ticket-id>`

**Example:** `/docs-update CS-1-1`

---

#### `/validate`
**Purpose:** Check which quality gates have passed. Read-only.

**What it does:**
1. Reads `docs/.active_ticket` if no ticket ID provided
2. Locates all gate artifacts for the ticket
3. Evaluates all 7 gates and reports status
4. For releases, checks all constituent tickets

**Usage:** `/validate [ticket-id]`

**Example:** `/validate CS-1-1` or `/validate R-3.22`

---

#### `/release`
**Purpose:** Bump workspace version, update CHANGELOG, commit, and tag.

**What it does:**
1. Validates argument is `patch`, `minor`, or `major`
2. Reads current version from `[workspace.package]` in `Cargo.toml`
3. Runs pre-release checks: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
4. Verifies git working tree is clean
5. Verifies `## [Unreleased]` section has content
6. Updates `Cargo.toml` version and regenerates `Cargo.lock`
7. Moves unreleased entries to a dated section in `CHANGELOG.md`
8. Commits `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`
9. Creates annotated git tag `vX.Y.Z`

**Usage:** `/release <patch|minor|major>`

**Example:** `/release minor`

---

#### `/sync-phases`
**Purpose:** Sync task completion status between `docs/tasklist.md` and individual `docs/phase/phase-*.md` files.

**What it does:**
1. Reads `docs/tasklist.md` for all phases
2. Reads each existing `docs/phase/phase-N.md`
3. Propagates `[x]` completions from phase files into the tasklist Feature Phases table
4. Creates a `phase-N.md` file for the first incomplete phase if it doesn't exist
5. Updates the `**Current Phase:**` line in `docs/tasklist.md`

**Usage:** `/sync-phases`

---

### Rust Knowledge Skills

These skills provide Rust-specific guidance and are triggered automatically when relevant:

#### `/rust-best-practices`
**Purpose:** Idiomatic Rust guidance based on Apollo GraphQL's best practices handbook.

**Triggers automatically when:** writing new Rust code, reviewing or refactoring, choosing between borrowing vs cloning, implementing error handling, optimizing performance, writing tests.

**Covers:** ownership/borrowing, `anyhow` error handling, performance with `--release`, clippy lints, testing patterns, generics vs `dyn Trait`, type state pattern, documentation style.

---

#### `/rust-async-patterns`
**Purpose:** Production patterns for async Rust with Tokio.

**Note for `ast-index`:** This project is intentionally synchronous (no `async`/`await`). This skill is available for reference but async patterns should not be introduced.

**Covers:** concurrent task execution, channels (`mpsc`, `broadcast`, `oneshot`, `watch`), error handling, graceful shutdown, async traits, streams.

---

#### `/rust-learner`
**Purpose:** Fetch Rust version info and crate documentation from authoritative sources.

**Triggers automatically when:** asking about latest Rust version, crate changelogs, API documentation, `Cargo.toml` dependency versions.

**Sources:** releases.rs, lib.rs, crates.io, docs.rs, doc.rust-lang.org.

---

#### `/rust-refactor-helper`
**Purpose:** Safe refactoring with LSP-backed impact analysis.

**Triggers automatically when:** renaming symbols, moving functions, extracting code.

**Actions:** `rename <old> <new>`, `extract-fn <file:line-range>`, `inline <fn>`, `move <symbol> <dest>`

**Usage:** `/rust-refactor-helper <action> <target> [--dry-run]`

**Example:** `/rust-refactor-helper rename parse_symbol index_symbol --dry-run`

---

## CI/CD

### Release Workflow (`release.yml`)

Triggered on `v*.*.*` tags. Builds `ast-index` binaries for 4 platforms:
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`

Artifacts are attached to the GitHub Release created by the tag.

### Plugin Branch Sync Workflow

Syncs `.claude/skills/` changes to the plugin distribution branch automatically on push to `main`.

### Pre-commit Checks (Required)

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

The `/release` command runs these automatically before bumping the version. Do not skip them.

---

## Error Handling and Recovery

### Common Errors and Solutions

#### Missing Ticket ID

**Error:** `Error: No ticket specified. Provide a ticket ID as a parameter or set it in docs/.active_ticket`

**Cause:** Command invoked without a ticket ID and `docs/.active_ticket` is empty or missing.

**Solution:**
```bash
# Option 1: Provide ticket ID explicitly
/plan CS-1-1

# Option 2: Set the active ticket first
echo "CS-1-1" > docs/.active_ticket
/plan
```

---

#### Plan Not Approved

**Error:** `Error: Plan for ticket <ticket> is not approved. Run /plan to create and approve the plan first.`

**Cause:** Attempted to run `/tasklist` before the plan was approved.

**Solution:**
1. Run `/plan CS-1-N` to create or complete the plan
2. Ensure the plan contains `Status: PLAN_APPROVED`
3. Re-run `/tasklist`

---

#### Prerequisite Artifact Missing

**Error:** Gate fails because a required artifact doesn't exist.

**Cause:** Attempted to skip a phase in the workflow.

**Solution:**
1. Run `/validate <ticket-id>` to check which gates have passed
2. Execute the missing phase commands in order
3. Resume from where you left off

---

### Implementation Failures

The `/implement-orchestrated` and `/quick-implement` commands have built-in error handling:

#### Verification Failure

**Behavior:** When `cargo fmt --check`, `cargo clippy -- -D warnings`, or `cargo test` fails:
1. Errors are parsed to identify the source (production code vs test code)
2. The responsible component is re-invoked with error context
3. Verification runs again
4. `/implement-orchestrated` retries up to **3 refinement iterations**; `/quick-implement` retries builds 3 times and review fixes 2 times

**What you'll see:**
```
Verification failed: cargo test returned errors
Iteration 1/3: Attempting refinement...
[error details]
Re-running verification...
```

---

#### Stuck Refinement (Same Errors Repeating)

**Behavior:** If identical errors occur for 2 consecutive iterations, the system detects it's stuck.

**Action taken:**
1. Changes are rolled back via `git stash pop` or `git restore .`
2. Error message displayed with last errors
3. Workflow terminates, requiring manual intervention

**What you'll see:**
```
Refinement stuck: Same errors detected for 2 iterations.
Rolling back changes...
Manual intervention required.
Last errors:
[error details]
```

---

#### Max Refinements Reached

**Behavior:** After max failed refinement attempts:

**Action taken:**
1. All changes are rolled back to the savepoint
2. Summary of attempts and final errors displayed
3. Workflow terminates (or asks whether to skip/stop for `/quick-implement`)

**Recovery:**
1. Review the error messages carefully
2. Manually fix the issue in code
3. Re-run the implement command to continue

---

#### Git Stash/Restore Failure

**Behavior:** If git operations fail during rollback:

**Manual recovery:**
```bash
# Check git status
git status

# Option 1: Discard all changes
git restore .
git clean -fd

# Option 2: If stash exists
git stash list
git stash pop

# Option 3: Hard reset to last commit (destructive)
git reset --hard HEAD
```

---

### Ambiguity and Clarification

#### Code Implementation Ambiguity

**Behavior:** When requirements are unclear during implementation, the workflow pauses and asks for clarification.

**What you'll see:**
```
Ambiguity detected: [description of unclear requirement]
Please clarify: [specific question]
```

**Action:** Provide the requested information to continue.

---

#### Production Bug Found During Testing

**Behavior:** If test writing reveals a bug in the production code:

**What you'll see:**
```
Potential production bug detected:
[description]

How would you like to proceed?
1. Fix the production code
2. Adjust the test expectations
3. Skip this test
```

**Action:** Choose the appropriate resolution.

---

### Description File Sync Errors

When using `/feature-development` or `/dev-cycle` with a description file (e.g., `docs/phase/phase-1.md`):

#### Task Mismatch

**Error:** `Error: The following tasks from the description file were not completed: [list]`

**Cause:** Tasks in the phase file don't match completed tasks in the tasklist.

**Solution:**
1. Review the listed tasks
2. Either complete the missing tasks or update the phase file
3. Re-run the workflow

---

### Recovery Procedures Summary

| Situation | Recovery Command/Action |
|-----------|------------------------|
| Unknown current state | `/validate CS-1-N` |
| Need to restart implementation | `git restore . && /implement-orchestrated CS-1-N` |
| Stuck on a specific task | Fix manually, mark `[x]` in tasklist, continue |
| Wrong changes committed | `git revert HEAD` or `git reset --soft HEAD~1` |
| Need to re-plan | Delete `docs/plan/<ticket>.md`, run `/plan` |
| Corrupted artifacts | Delete affected files, re-run corresponding phase |
| `/phase-loop` stuck on same phase | Fix the task manually, mark `[x]` in phase file, re-run |

---

### Checkpoint Confirmations (`/feature-development` only)

The `/feature-development` command asks for confirmation at three points:

1. **After PRD:** "PRD has been created. Ready to proceed to research and planning phase?"
2. **After plan:** "Implementation plan has been created. Ready to proceed to task breakdown and implementation?"
3. **After implementation:** "Implementation is complete. Ready to proceed to code review and QA?"

Each checkpoint offers "Continue" or "Pause to review". `/dev-cycle` and `/phase-loop` have **no checkpoints** — they run fully automatically.

---

## Ticket ID Convention

- **Feature tickets:** `CS-N-N` (e.g., `CS-1-1`, `CS-1-2`, `CS-2-1`)
  - First number = epic/phase group, second number = ticket within that group
- **Releases:** `R-X.Y` (e.g., `R-3.22`, `R-4.0`)

The active ticket is stored in `docs/.active_ticket`. Most commands will read from this file if no ticket ID is provided.

Branch naming: `feature/<ticket>-<short-description>` (e.g., `feature/cs-1-phase1`)

## Running the Full Workflow

```bash
# Option 1: Full orchestration with user checkpoints
/feature-development CS-1-2 docs/phase/phase-2.md

# Option 2: Fully automatic (no checkpoints)
/dev-cycle CS-1-2 docs/phase/phase-2.md

# Option 3: Phase-based loop (uses docs/tasklist.md)
/phase-loop
/phase-loop --no-commit   # Disable auto-commit between phases

# Option 4: Individual phases (manual control)
/analysis CS-1-2 "Flutter Parser" docs/phase/phase-2.md
/research CS-1-2
/plan CS-1-2
/tasklist CS-1-2
/implement-orchestrated CS-1-2
/run-reviewer CS-1-2
/qa CS-1-2
/docs-update CS-1-2
/validate CS-1-2

# Option 5: Phase file direct execution
/quick-implement docs/phase/phase-2.md

# Release
/release minor
```

## Tips

1. **Check status first:** Use `/validate <ticket>` to see which gates have passed
2. **Description files:** Phase files (`docs/phase/phase-N.md`) can be passed as the second argument to `/feature-development`, `/dev-cycle`, or `/analysis` for additional context
3. **Rollback safety:** `/implement-orchestrated` creates git savepoints before changes; `/quick-implement` relies on git for rollback
4. **Incremental progress:** Run individual commands to advance one gate at a time
5. **Active ticket:** Set `docs/.active_ticket` once, then omit ticket IDs from subsequent commands
6. **Recovery:** When in doubt, run `/validate` to understand current state before proceeding
7. **Pre-release checks:** `cargo fmt --check && cargo clippy -- -D warnings && cargo test` must pass before `/release` runs — fix any issues first
8. **Phase loop vs dev-cycle:** Use `/phase-loop` when work is organized in `docs/tasklist.md` phases; use `/dev-cycle` for single-ticket work with a standard PRD → plan → tasklist flow
9. **Rust knowledge:** `/rust-best-practices` and `/rust-refactor-helper` trigger automatically — no need to call them explicitly
