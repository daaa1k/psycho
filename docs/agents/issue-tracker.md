# Issue tracker: GitHub

Issues and specs for this repo live as GitHub issues. Use the `gh` CLI for all operations.

## Conventions

- **Create an issue**: `gh issue create --title "..." --body-file <file>`.
- **Read an issue**: `gh issue view <number> --comments`.
- **List issues**: `gh issue list --state open --json number,title,body,labels,assignees` with appropriate filters.
- **Comment on an issue**: `gh issue comment <number> --body-file <file>`.
- **Apply / remove labels**: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`.
- **Close**: `gh issue close <number> --comment "..."`.

Infer the repo from `git remote -v`; `gh` does this automatically inside the clone.

## Pull requests as a triage surface

**PRs as a request surface: no.**

## When a skill says "publish to the issue tracker"

Create a GitHub issue.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --comments`.

## Wayfinding operations

Used by `/wayfinder`. The map is a single issue with child issues as tickets.

- **Map**: an issue labelled `wayfinder:map`, holding Destination, Notes, Decisions so far, Not yet specified, and Out of scope.
- **Child ticket**: a GitHub sub-issue of the map, labelled `wayfinder:research`, `wayfinder:prototype`, `wayfinder:grilling`, or `wayfinder:task`. Use `gh api` on the sub-issues endpoint. If sub-issues are unavailable, add a task list to the map and `Part of #<map>` to each child body.
- **Blocking**: use GitHub's native issue dependencies. Add an edge with `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`. Obtain the blocker's database ID with `gh api repos/<owner>/<repo>/issues/<number> --jq .id`. If dependencies are unavailable, use a `Blocked by: #<number>` line in the ticket body.
- **Frontier**: inspect the map's open children. A ticket is available if it has no open blocker and no assignee; first in map order wins.
- **Claim**: `gh issue edit <number> --add-assignee @me` before working on the ticket.
- **Resolve**: post the answer as a comment, close the ticket, then append a linked one-line gist to the map's Decisions so far.
