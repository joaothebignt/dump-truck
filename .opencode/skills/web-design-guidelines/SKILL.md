---
name: web-design-guidelines
description: Review UI code for Web Interface Guidelines compliance. Use when asked to "review my UI", "check accessibility", "audit design", "review UX", or "check my site against best practices". Audits code for 100+ rules covering accessibility, performance, and UX.
license: MIT
compatibility: opencode
metadata:
  audience: ui developers
  workflow: code-review
  origin: vercel-labs/agent-skills skills/web-design-guidelines
  argument-hint: <file-or-pattern>
---

Review files for compliance with Web Interface Guidelines.

## What I Do

- Fetch the latest guidelines from the source URL below
- Read the specified files (or prompt user for files/pattern)
- Check against all rules in the fetched guidelines
- Output findings in the terse `file:line` format

## Guidelines Source

Fetch fresh guidelines before each review:

```
https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md
```

Use the agent's HTTP/fetch capability to retrieve the latest rules. The fetched content contains all the rules and output format instructions.

## Workflow

When a user provides a file or pattern argument:

1. Fetch guidelines from the source URL above
2. Read the specified file(s) or glob pattern
3. Check each file against all fetched rules
4. Report violations in `file:line: [RULE] description` format

When no argument is provided:

1. Ask the user: "Which file or pattern would you like me to review? (e.g. `src/components/*.tsx`)"
2. Proceed with the same steps above once provided

## Output Format

```
src/components/Button.tsx:14: [A11Y] Button is missing an accessible label
src/pages/index.tsx:32: [PERF] Image missing width/height attributes
src/pages/index.tsx:58: [UX] Form input missing associated label element
```

Keep output terse. Group by file. Only report violations — do not list passing rules.
