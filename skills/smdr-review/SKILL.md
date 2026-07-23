# smdr-review — Agent Skill

Open any Markdown content in **smdr's interactive review mode**, wait for the
user to annotate it line-by-line, then read the structured JSON feedback and
act on it.

This bridges the gap between automated agent work and human judgement: instead
of dumping a wall of text in the chat and hoping the user responds clearly,
you open a polished native viewer, the user clicks line numbers to leave
pinned comments, and you get back a clean JSON structure that tells you exactly
which line each comment refers to.

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
plain text can be left as-is (smdr renders it fine), structured data should
become a Markdown table or fenced code block, and headings help the user
navigate long documents.

```bash
REVIEW_FILE=$(mktemp /tmp/smdr-review-XXXXXX.md)
cat > "$REVIEW_FILE" << 'EOF'
# Plan — <short title>

<your markdown content here>
EOF
```

Give the file a meaningful title (`# H1`) so the user immediately understands
what they are reviewing. Use `##` sections to separate logical groups (e.g.
"## Phase 1", "## Risks", "## Open questions"). This makes the gutter line
numbers more meaningful.

### Step 2 — Open smdr in review mode

```bash
smdr --review "$REVIEW_FILE"
```

smdr opens a native window. The user can:
- Click any **gutter line number** to leave a comment on that line.
- Press `c` to toggle between rendered and raw-source view.
- Press `Ctrl-Enter` (or click **Submit review**) when done — this writes
  the JSON envelope to stdout and exits.

Capture the output:

```bash
FEEDBACK_JSON=$(smdr --review "$REVIEW_FILE")
```

Or if you need to persist it:

```bash
smdr --review --out /tmp/smdr-feedback.json "$REVIEW_FILE"
FEEDBACK_JSON=$(cat /tmp/smdr-feedback.json)
```

### Step 3 — Parse and act on the feedback

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
| `schema` | Always `"smdr.review/v1"` — useful for version-gating |
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
- **Comments on specific lines** — read the source file to find what line `N`
  contains, then address each comment in turn. Map the 0-based line index back
  to the file with `sed -n "$((line+1))p" "$REVIEW_FILE"`.
- **Blocking concerns** — surface them to the user in chat and ask how to
  resolve before continuing.
- **Approval comments** ("looks good", "✓", "approved") — treat as a green
  light for that section.

### Step 4 — Clean up

```bash
rm -f "$REVIEW_FILE" /tmp/smdr-feedback.json
```

Draft files are auto-saved by smdr to `$TMPDIR/smdr-drafts/` until submitted,
so the user can safely close and reopen the window without losing work. They
are cleared automatically on submit.

---

## Helper: convert a structured list to Markdown

If you have a plain-text todo list or a data structure, convert it before
passing to smdr:

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

md = "\n".join(lines)

with tempfile.NamedTemporaryFile(suffix=".md", delete=False, mode="w") as f:
    f.write(md)
    path = f.name

subprocess.run(["smdr", "--review", "--out", "/tmp/smdr-fb.json", path])
feedback = json.loads(open("/tmp/smdr-fb.json").read())
os.unlink(path)
os.unlink("/tmp/smdr-fb.json")

for c in feedback["comments"]:
    print(f'Line {c["line"]}: {c["comment"]}')
```

---

## Headless / non-interactive path

If you already have an annotations file and want to render a report without
opening a window (e.g. in CI):

```bash
smdr --review --annotations-in annotations.json --format md --out report.md plan.md
```

Formats: `json` (default), `md` (annotated Markdown), `diff` (unified diff).

The annotations file uses the same `ReviewEnvelope` schema shown above.

---

## Checklist

Before calling `smdr --review`, verify:

- [ ] `smdr` is on PATH (`which smdr` succeeds)
- [ ] The temp file is valid Markdown with a top-level `# Title`
- [ ] A display is available — smdr opens a native window (X11/Wayland/macOS);
      use the `--annotations-in` path in headless/CI environments
- [ ] You are capturing stdout or using `--out` to save the JSON feedback
