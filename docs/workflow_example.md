# Feature Development Workflow

This document describes the end-to-end AI-driven feature development workflow used in this project.

## Overview

Features progress through a series of quality gates, each producing specific artifacts. The workflow is orchestrated by the `/feature-development` command, which invokes specialized slash commands for each phase.

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
├── .active_ticket        # Current ticket ID
├── prd/
│   └── <ticket>.prd.md   # Product Requirements Document
├── research/
│   └── <ticket>.md       # Technical research findings
├── plan/
│   └── <ticket>.md       # Architecture and implementation plan
├── tasklist/
│   └── <ticket>.md       # Breakdown of tasks with checkboxes
├── adr/
│   └── <ticket>.md       # Architecture Decision Records (if alternatives exist)
├── phase/
│   └── phase-N.md        # Individual phase files for iterations
└── releases/
    └── <release>.md      # Release bundles (R-prefixed IDs)

reports/
└── qa/
    └── <ticket>.md       # QA plan and verdict
```

## Slash Commands Reference

### Orchestration

| Command | Description | Usage |
|---------|-------------|-------|
| `/feature-development` | End-to-end workflow orchestrator. Runs all gates in sequence until complete. | `/feature-development <ticket-id> "<title>" [description-file]` |
| `/validate` | Checks which quality gates have passed for a ticket or release. Read-only analysis. | `/validate <ticket-id>` |
| `/sync-phases` | Syncs task completion status between `tasklist.md` and individual `phase-*.md` files. | `/sync-phases` |

### Phase Commands

Run these individually when you need fine-grained control:

#### `/analysis`
**Purpose:** Initialize a feature ticket and create the PRD.

**What it does:**
1. Sets `docs/.active_ticket` to the ticket ID
2. Creates `docs/prd/<ticket>.prd.md` from template
3. Populates sections: goals, user stories, scenarios, metrics, constraints, risks, open questions
4. Sets status to `DRAFT` or `PRD_READY`

**Usage:** `/analysis <ticket-id> "<title>" [description-file]`

**Example:** `/analysis SY-3 "Plugin Loading" docs/phase/phase-3.md`

---

#### `/research`
**Purpose:** Gather technical context before planning.

**What it does:**
1. Reads the PRD and asks clarifying questions via interactive prompts
2. Scans codebase for related entities, patterns, and dependencies
3. Documents findings in `docs/research/<ticket>.md`
4. Does NOT modify code

**Usage:** `/research <ticket-id>`

**Example:** `/research SY-2`

---

#### `/plan`
**Purpose:** Create architecture and implementation plan.

**What it does:**
1. Reads PRD, research doc, and conventions
2. Creates `docs/plan/<ticket>.md` with: components, API contract, data flows, NFRs, risks
3. Creates `docs/adr/<ticket>.md` if architectural alternatives exist
4. Sets `Status: PLAN_APPROVED` when complete

**Usage:** `/plan <ticket-id>`

**Example:** `/plan SY-2`

---

#### `/tasklist`
**Purpose:** Break down the plan into actionable tasks.

**What it does:**
1. Requires `PLAN_APPROVED` status
2. Creates `docs/tasklist/<ticket>.md` with checkbox tasks
3. Each task has 1-2 acceptance criteria
4. Sets `Status: TASKLIST_READY` when complete

**Usage:** `/tasklist <ticket-id>`

**Example:** `/tasklist SY-2`

---

#### `/implement-orchestrated`
**Purpose:** Implement tasks with code/test separation and automated verification.

**What it does:**
1. Finds first incomplete task `- [ ]` in tasklist
2. Creates git savepoint for rollback
3. Implements production code
4. Optionally writes tests
5. Runs verification: `cargo fmt`, `cargo check`, `cargo test`, `cargo clippy`
6. Refines up to 3 times if verification fails
7. Marks task complete `- [x]` on success
8. Rolls back on failure after max iterations

**Usage:** `/implement-orchestrated <ticket-id>`

**Example:** `/implement-orchestrated SY-2`

---

#### `/run-reviewer`
**Purpose:** Review code changes against requirements.

**What it does:**
1. Reads PRD, plan, tasklist, and conventions
2. Analyzes diff for ticket-related changes
3. Generates review with: blocking issues, important recommendations, cosmetic notes
4. Suggests additional tasks if gaps found

**Usage:** `/run-reviewer <ticket-id>`

**Example:** `/run-reviewer SY-2`

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

**Example:** `/qa SY-2` or `/qa R-1.0`

---

#### `/docs-update`
**Purpose:** Update documentation after implementation.

**What it does:**
1. Reads all artifacts and code diff
2. Creates `<ticket>-summary.md` documenting decisions
3. Adds entry to `CHANGELOG.md`

**Usage:** `/docs-update <ticket-id>`

**Example:** `/docs-update SY-2`

---

## Error Handling and Recovery

### Common Errors and Solutions

#### Missing Ticket ID

**Error:** `Error: No ticket specified. Provide a ticket ID as a parameter or set it in docs/.active_ticket`

**Cause:** Command invoked without a ticket ID and `docs/.active_ticket` is empty or missing.

**Solution:**
```bash
# Option 1: Provide ticket ID explicitly
/plan SY-2

# Option 2: Set the active ticket first
echo "SY-2" > docs/.active_ticket
/plan
```

---

#### Plan Not Approved

**Error:** `Error: Plan for ticket <ticket> is not approved. Run /plan to create and approve the plan first.`

**Cause:** Attempted to run `/tasklist` before the plan was approved.

**Solution:**
1. Run `/plan SY-N` to create or complete the plan
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

The `/implement-orchestrated` command has built-in error handling:

#### Verification Failure

**Behavior:** When `cargo check`, `cargo test`, or `cargo clippy` fails:
1. Errors are parsed to identify the source (production code vs test code)
2. The responsible component is re-invoked with error context
3. Verification runs again
4. This repeats up to **3 refinement iterations**

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

**Behavior:** After 3 failed refinement attempts:

**Action taken:**
1. All changes are rolled back to the savepoint
2. Summary of attempts and final errors displayed
3. Workflow terminates

**What you'll see:**
```
Refinement failed after 3 attempts.
Changes have been rolled back.
Manual intervention required.

Last errors:
[error details]
```

**Recovery:**
1. Review the error messages carefully
2. Manually fix the issue in code
3. Run `/implement-orchestrated` again to continue

---

#### Git Stash/Restore Failure

**Behavior:** If git operations fail during rollback:

**What you'll see:**
```
Error: Git restore failed. Please resolve manually.
[git error details]
```

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

When using `/feature-development` with a description file (e.g., `docs/phase/phase-2.md`):

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
| Unknown current state | `/validate SY-N` |
| Need to restart implementation | `git restore . && /implement-orchestrated SY-N` |
| Stuck on a specific task | Fix manually, mark `[x]` in tasklist, continue |
| Wrong changes committed | `git revert HEAD` or `git reset --soft HEAD~1` |
| Need to re-plan | Delete `docs/plan/<ticket>.md`, run `/plan` |
| Corrupted artifacts | Delete affected files, re-run corresponding phase |

---

### Checkpoint Confirmations

The `/implement-orchestrated` command requires explicit confirmation at these points:

1. **Before implementing:** "Proceed with this approach?"
2. **Before writing tests:** "Write tests for this implementation?"
3. **Before next task:** "Continue to next task?"

These checkpoints allow you to:
- Review the proposed changes before they're made
- Skip tests if not needed for a particular task
- Pause between tasks to review progress

---

## Ticket ID Convention

- **Feature tickets:** `SY-N` (e.g., `SY-1`, `SY-2`)
- **Releases:** `R-X.Y` (e.g., `R-1.0`, `R-2.0`)

The active ticket is stored in `docs/.active_ticket`. Most commands will read from this file if no ticket ID is provided.

## Running the Full Workflow

```bash
# Option 1: Run the full orchestrated workflow
/feature-development SY-2 "CLI Arguments" docs/phase/phase-2.md

# Option 2: Run individual phases manually
/analysis SY-2 "CLI Arguments" docs/phase/phase-2.md
/research SY-2
/plan SY-2
/tasklist SY-2
/implement-orchestrated SY-2
/run-reviewer SY-2
/qa SY-2
/docs-update SY-2
/validate SY-2
```

## Tips

1. **Check status first:** Use `/validate <ticket>` to see which gates have passed
2. **Description files:** Phase files (`docs/phase/phase-N.md`) can be passed as the third argument to `/feature-development` or `/analysis` for additional context
3. **Rollback safety:** `/implement-orchestrated` creates git savepoints before changes
4. **Incremental progress:** You can run individual commands to advance one gate at a time
5. **Active ticket:** Set `docs/.active_ticket` once, then omit ticket IDs from subsequent commands
6. **Recovery:** When in doubt, run `/validate` to understand current state before proceeding
