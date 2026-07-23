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

```bash
### Step 2 — Open smdr in review mode

smdr blocks until the user submits, then writes the JSON envelope to stdout
and exits. Capture it directly:

```bash
FEEDBACK_JSON=$(smdr --review "$REVIEW_FILE")
```

The output is a `ReviewEnvelope` JSON object:

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

Parse with `jq`:

```bash
echo "$FEEDBACK_JSON" | jq -r '.comments[] | "Line \(.line): \(.comment)"'
```

Or with Python:

```python
import json, sys
env = json.load(sys.stdin)
for c in env["comments"]:
    print(f'Line {c["line"]}: {c["comment"]}')
```

**What to do with the feedback:**

- **No comments** — the user is happy; proceed.
- **Comments on specific lines** — map the 0-based line index back to source
  content with `sed -n "$((line+1))p" "$REVIEW_FILE"`, then address each comment.
- **Blocking concerns** — surface them to the user in chat and ask how to
  resolve before continuing.
- **Approval comments** ("looks good", "✓", "approved") — treat as a green
  light for that section.

### Step 4 — Clean up

```bash
rm -f "$REVIEW_FILE"
```

---

## Helper: convert a structured list to Markdown

```python
import subprocess, tempfile, json, os

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
os.unlink(path)

---

## Headless / non-interactive path

If you already have an annotations file and want to render a report without
opening a window (e.g. in CI):

```bash
smdr --review --annotations-in annotations.json --format md plan.md
```

Formats: `json` (default), `md` (annotated Markdown), `diff` (unified diff).
The annotations file uses the same `ReviewEnvelope` schema shown above.
