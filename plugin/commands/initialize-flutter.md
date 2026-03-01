---
name: initialize-flutter
description: Initialize ast-index for Dart/Flutter project - configures .claude/settings.json, rules, and CLAUDE.md
---

# Initialize ast-index for Flutter Project

This command sets up ast-index integration for a Dart/Flutter project.

## Steps to Execute

### 1. Check Prerequisites

Verify ast-index is installed:

```bash
ast-index version
```

If not installed, inform user to run:
```bash
brew tap defendend/ast-index
brew install ast-index
```

### 2. Create/Update .claude/settings.json

First, ensure the directory exists:

```bash
mkdir -p .claude
```

Then create or merge into `.claude/settings.json`. If file doesn't exist, create it with this content:

```json
{
  "extraKnownMarketplaces": {
    "ast-index": {
      "source": {
        "source": "github",
        "repo": "defendend/Claude-ast-index-search"
      }
    }
  },
  "enabledPlugins": {
    "ast-index@ast-index": true
  },
  "permissions": {
    "allow": [
      "Bash(ya tool ast-index *)",
      "Bash(ast-index *)"
    ]
  }
}
```

**Important**: If `.claude/settings.json` already exists, MERGE the keys (don't replace the whole file).

### 3. Create .claude/rules/ast-index.md (CRITICAL)

Create the rules directory and ast-index rules file:

```bash
mkdir -p .claude/rules
```

Create file `.claude/rules/ast-index.md` with this content:

```markdown
# ast-index Rules

## Mandatory Search Rules

1. **ALWAYS use ast-index FIRST** for any code search task
2. **NEVER duplicate results** — if ast-index found usages/implementations, that IS the complete answer
3. **DO NOT run grep "for completeness"** after ast-index returns results
4. **Use grep/Search ONLY when:**
   - ast-index returns empty results
   - Searching for regex patterns (ast-index uses literal match)
   - Searching for string literals inside code (`"some text"`)
   - Searching in comments content

## Why ast-index

ast-index is 17-69x faster than grep (1-10ms vs 200ms-3s) and returns structured, accurate results.

## Command Reference

| Task | Command | Time |
|------|---------|------|
| Universal search | `ast-index search "query"` | ~10ms |
| Find class/mixin | `ast-index class "ClassName"` | ~1ms |
| Find symbol | `ast-index symbol "SymbolName"` | ~1ms |
| Find usages | `ast-index usages "SymbolName"` | ~8ms |
| Find implementations | `ast-index implementations "Parent"` | ~5ms |
| Call hierarchy | `ast-index call-tree "function" --depth 3` | ~1s |
| Class hierarchy | `ast-index hierarchy "ClassName"` | ~5ms |
| Find callers | `ast-index callers "functionName"` | ~1s |
| Module deps | `ast-index deps "module-name"` | ~10ms |
| File outline | `ast-index outline "widget.dart"` | ~1ms |

## Flutter-Specific Search Rules

- Use `ast-index implementations "StatelessWidget"` or `"StatefulWidget"` to find all widget implementations
- Use `ast-index implementations "Bloc"` or `"ChangeNotifier"` to find state management classes
- Use `ast-index class "Mixin"` to find mixins (indexed as Interface kind)
- Use `ast-index search "extension"` to find Dart extensions (indexed as Object kind)
- Use `ast-index hierarchy "Widget"` to explore the widget inheritance tree
- Use `ast-index conventions` to auto-detect BLoC, Provider, and other architecture patterns

## Flutter/Dart-Specific Commands

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

## Index Management

- `ast-index rebuild` — Full reindex (run once after clone)
- `ast-index update` — After git pull/merge
- `ast-index stats` — Show index statistics
```

### 4. Build the Index

Run initial indexing:

```bash
ast-index rebuild
```

Show progress and report statistics when done.

### 5. Verify Setup

Run a quick search to verify everything works:

```bash
ast-index stats
ast-index search "Widget"
```

## Output

After completion, inform user:
- settings.json has been configured with ast-index permissions
- Rules file created at .claude/rules/ast-index.md
- Index has been built with X files and Y symbols
- Ready to use ast-index for code search

## Flutter Project Detection

For Flutter projects, you can detect common architecture patterns:

```bash
# Check for BLoC pattern
ast-index implementations "Bloc"

# Check for Provider pattern
ast-index implementations "ChangeNotifier"

# Check for Riverpod
ast-index search "ConsumerWidget"

# Check project structure
ast-index conventions
```
