# smdr-review — Agent Skill

**smdr** is a native Markdown viewer with a built-in review mode: it opens a
file in a GUI window, collects line-anchored comments from the user, and emits
a structured JSON envelope on submit. Use it to gather precise human feedback
on any content an agent produces.

## First — verify smdr is installed

Before doing anything else, run:

```bash
which smdr
```

If the command is not found, stop and tell the user:

> smdr is not installed. Please follow the install instructions at
> https://santanusinha.github.io/smdr/#install

Do not proceed until `which smdr` succeeds.

---

## When to use this

- You have produced a plan, TODO list, proposal, design doc, or any multi-line
  content and want the user's structured, line-level sign-off before proceeding.
- An irreversible or high-impact action is about to happen and you want to
  confirm the steps first.
- The user asks to "review", "annotate", "comment on", or "give feedback on"
  a piece of content.
- You are acting as a subagent and your orchestrator asks you to collect
  human feedback before continuing.

---

## Workflow

### Step 1 — Write the content to a temp file

The content must be a Markdown file. If it isn't already Markdown, convert it:
plain text can be left as-is, structured data should become a Markdown table
or fenced code block, and headings help the user navigate long documents.

```bash
REVIEW_FILE=$(mktemp /tmp/smdr-review-XXXXXX.md)
cat > "$REVIEW_FILE" << 'EOF'
# Plan — <short title>

<your markdown content here>
EOF
```

Give the file a meaningful title (`# H1`). Use `##` sections to separate
logical groups (e.g. "## Phase 1", "## Risks", "## Open questions").

### Step 2 — Open smdr in review mode

smdr blocks until the user submits, then writes the JSON envelope to stdout
and exits. Capture it directly:

```bash
FEEDBACK=$(smdr --review "$REVIEW_FILE")
```

### Step 3 — Read the feedback

The output is a `ReviewEnvelope` JSON object — read it directly, no parsing
needed:

```json
{
  "schema": "smdr.review/v1",
  "file": "/tmp/smdr-review-abc123.md",
  "comments": [
    { "line": 4,  "comment": "This step is missing the rollback plan." },
    { "line": 11, "comment": "Looks good." }
  ]
}
```

| Field | Meaning |
|---|---|
| `schema` | Always `"smdr.review/v1"` |
| `file` | Path to the file that was reviewed |
| `comments[].line` | **0-based** source line the comment is anchored to |
| `comments[].comment` | The user's freeform note |

**What to do with the feedback:**

- **No comments** — the user is happy; proceed.
- **Comments on specific lines** — `comments[].line` is 0-based; find the
  corresponding line in the source file and address the comment.
- **Blocking concerns** — surface them to the user in chat and ask how to
  resolve before continuing.
- **Approval comments** ("looks good", "✓", "approved") — treat as a green
  light for that section.

---

## Helper: convert a structured list to Markdown

```python
import subprocess, tempfile, json

items = [
    {"phase": 1, "task": "Scaffold project", "risk": "low"},
    {"phase": 1, "task": "Set up CI",        "risk": "low"},
    {"phase": 2, "task": "Database schema",  "risk": "medium"},
]

lines = ["# Implementation Plan\n"]
current_phase = None
for item in items:
    if item["phase"] != current_phase:
        current_phase = item["phase"]
        lines.append(f"\n## Phase {current_phase}\n")
    lines.append(f"- [ ] {item['task']} _(risk: {item['risk']})_")

with tempfile.NamedTemporaryFile(suffix=".md", delete=False, mode="w") as f:
    f.write("\n".join(lines))
    path = f.name

feedback = json.loads(subprocess.check_output(["smdr", "--review", path]))
# feedback is a dict — read it directly
```

---

## Headless / non-interactive path

Use this when there is no display available (e.g. CI) or when you want to
produce a formatted report from an existing annotations file.

The annotations file is a `ReviewEnvelope` — the same JSON that smdr emits
from an interactive session. Save one with `--out`:

```bash
smdr --review --out annotations.json plan.md
```

Then render it into a report at any time, without opening a window:

```bash
smdr --review --annotations-in annotations.json --format md plan.md
```

Formats: `json` (default), `md` (annotated Markdown), `diff` (unified diff).