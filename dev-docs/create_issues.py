#!/usr/bin/env python3
"""
Create the Font Manager backlog on GitHub from issues.json.

What it creates, idempotently (safe to re-run):
  1. Labels and milestones
  2. One issue per epic (type:epic) and one per story (type:story)
  3. TRUE sub-issue relationships: every story is a sub-issue of its epic
     (POST /repos/{r}/issues/{epic}/sub_issues) — not a task-list link
  4. TRUE dependencies: every "depends" entry becomes a "blocked by" relationship
     (POST /repos/{r}/issues/{n}/dependencies/blocked_by)
  5. Optionally a Projects (v2) board with every issue added

Requirements: Python 3.9+, GitHub CLI (`gh`) installed and authenticated
(`gh auth login`) with a token that has Issues: write and Projects: write.

Usage:
  python create_issues.py --repo OWNER/NAME                # create everything
  python create_issues.py --repo OWNER/NAME --dry-run      # print the plan only
  python create_issues.py --repo OWNER/NAME --project      # also create the board
  python create_issues.py --repo OWNER/NAME --only E1 E2   # limit to some epics

Re-running never duplicates: issues are matched by the "E1.3 " prefix of their title.
A map of ID -> issue number/id is written to issues.map.json next to this script.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
API_VERSION = "2026-03-10"
ID_RE = re.compile(r"^(E\d+(?:\.\d+)?)\b")
WRITE_DELAY = 0.6  # seconds between write calls, to stay under secondary rate limits


# ----------------------------------------------------------------------------- gh helpers

def gh(args: list[str], payload: dict | None = None, ok_codes: tuple[int, ...] = (0,)) -> tuple[int, str]:
    """Run `gh` and return (exit_code, stdout). payload is sent as JSON on stdin."""
    cmd = ["gh", *args]
    if payload is not None:
        cmd += ["--input", "-"]
    proc = subprocess.run(
        cmd,
        input=json.dumps(payload) if payload is not None else None,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if proc.returncode not in ok_codes:
        sys.stderr.write(f"\n$ {' '.join(cmd)}\n{proc.stdout}\n{proc.stderr}\n")
    return proc.returncode, proc.stdout


def api(method: str, path: str, payload: dict | None = None, retries: int = 4) -> dict | list | None:
    """Call the REST API through gh with retries on secondary rate limits."""
    for attempt in range(retries):
        args = ["api", "-X", method, "-H", f"X-GitHub-Api-Version: {API_VERSION}",
                "-H", "Accept: application/vnd.github+json", path]
        code, out = gh(args, payload, ok_codes=(0, 1))
        if code == 0:
            return json.loads(out) if out.strip() else None
        if "rate limit" in out.lower() or "secondary" in out.lower() or "abuse" in out.lower():
            wait = 30 * (attempt + 1)
            print(f"   rate limited, waiting {wait}s …")
            time.sleep(wait)
            continue
        raise RuntimeError(f"gh api {method} {path} failed")
    raise RuntimeError(f"gh api {method} {path} failed after retries")


def api_paginated(path: str) -> list:
    code, out = gh(["api", "-H", f"X-GitHub-Api-Version: {API_VERSION}", "--paginate", "--slurp", path])
    if code != 0:
        raise RuntimeError(f"gh api --paginate {path} failed")
    pages = json.loads(out) if out.strip() else []
    items: list = []
    for page in pages:
        items.extend(page if isinstance(page, list) else [page])
    return items


# ----------------------------------------------------------------------------- body builders

def meta_block(**fields) -> str:
    """Machine-readable block the harness parses; kept in an HTML comment so it does not render."""
    lines = ["<!-- fm-meta"]
    for k, v in fields.items():
        if isinstance(v, list):
            v = "[" + ", ".join(v) + "]"
        elif isinstance(v, bool):
            v = "true" if v else "false"
        lines.append(f"{k}: {v}")
    lines.append("-->")
    return "\n".join(lines)


def story_body(story: dict, epic: dict, project: dict) -> str:
    deps = story.get("depends", [])
    parts = [f"**Epic:** {epic['id']} {epic['title']}  ",
             f"**Size:** {story['size']}  ",
             f"**Track:** {story.get('track', epic.get('track', 'core'))}  ",
             f"**Depends on:** {', '.join(deps) if deps else '—'}", ""]
    if story.get("human"):
        parts += ["> **Needs a human.** Spike, decision or interactive setup; the harness skips this story.", ""]
    parts += ["## Acceptance criteria", ""]
    if story.get("criteria"):
        parts += [f"- [ ] {c}" for c in story["criteria"]]
    else:
        parts += ["- [ ] _To be written at the end of the previous phase, when the shape of the code is known._"]
    parts += ["", "## Definition of done", "",
              "- [ ] Merged to `main` with a PR that references this issue",
              "- [ ] Unit tests for the core logic; CI matrix green on Windows, macOS and Linux",
              "- [ ] Acceptance criteria demonstrated on the reference library",
              "- [ ] Docs updated where a user-facing behavior changed", "",
              f"Spec: {project['spec_url']}  ", f"Breakdown: {project['breakdown_url']}", "",
              meta_block(id=story["id"], epic=epic["id"], phase=epic["phase"], size=story["size"],
                         track=story.get("track", epic.get("track", "core")),
                         human=bool(story.get("human")), depends=deps)]
    return "\n".join(parts)


def epic_body(epic: dict, project: dict) -> str:
    deps = epic.get("depends", [])
    parts = [epic.get("summary", ""), "",
             f"**Phase:** {epic['phase']}  ", f"**Track:** {epic.get('track', 'core')}  ",
             f"**Depends on:** {', '.join(deps) if deps else '—'}", "",
             "## Stories", "",
             "_Attached below as sub-issues; the list here is for reading only._", ""]
    parts += [f"- {s['id']} {s['title']} ({s['size']})" for s in epic["stories"]]
    parts += ["", f"Spec: {project['spec_url']}  ", f"Breakdown: {project['breakdown_url']}", "",
              meta_block(id=epic["id"], phase=epic["phase"], track=epic.get("track", "core"), depends=deps)]
    return "\n".join(parts)


def story_labels(story: dict, epic: dict) -> list[str]:
    labels = ["type:story", f"phase:{epic['phase']}", f"size:{story['size']}",
              f"track:{story.get('track', epic.get('track', 'core'))}"]
    if story.get("human"):
        labels.append("human")
    if not story.get("criteria"):
        labels.append("criteria:tbd")
    if not story.get("depends") and not story.get("human") and story.get("criteria"):
        labels.append("harness:ready")
    return labels


# ----------------------------------------------------------------------------- main steps

def ensure_labels(repo: str, labels: list[dict], dry: bool) -> None:
    print(f"Labels ({len(labels)})")
    for lb in labels:
        print(f"   {lb['name']}")
        if dry:
            continue
        code, _ = gh(["label", "create", lb["name"], "--repo", repo, "--color", lb["color"],
                      "--description", lb.get("description", ""), "--force"])
        if code != 0:
            raise RuntimeError(f"label {lb['name']} failed")


def ensure_milestones(repo: str, milestones: list[dict], dry: bool) -> dict[str, int]:
    print(f"Milestones ({len(milestones)})")
    existing = {} if dry else {m["title"]: m["number"]
                               for m in api_paginated(f"repos/{repo}/milestones?state=all&per_page=100")}
    numbers: dict[str, int] = {}
    for ms in milestones:
        if ms["title"] in existing:
            print(f"   = {ms['title']}")
            numbers[ms["title"]] = existing[ms["title"]]
            continue
        print(f"   + {ms['title']}")
        if dry:
            numbers[ms["title"]] = -1
            continue
        created = api("POST", f"repos/{repo}/milestones", {"title": ms["title"], "description": ms.get("description", "")})
        numbers[ms["title"]] = created["number"]
        time.sleep(WRITE_DELAY)
    return numbers


def load_existing_issues(repo: str) -> dict[str, dict]:
    """Map 'E3.8' -> issue for every issue whose title starts with an ID."""
    found: dict[str, dict] = {}
    for issue in api_paginated(f"repos/{repo}/issues?state=all&per_page=100"):
        if "pull_request" in issue:
            continue
        m = ID_RE.match(issue["title"])
        if m:
            found[m.group(1)] = issue
    return found


def create_or_get(repo: str, existing: dict[str, dict], issue_id: str, title: str, body: str,
                  labels: list[str], milestone: int, dry: bool) -> dict:
    if issue_id in existing:
        print(f"   = #{existing[issue_id]['number']} {title}")
        return existing[issue_id]
    print(f"   + {title}")
    if dry:
        fake = {"number": -1, "id": -1, "html_url": "", "title": title}
        existing[issue_id] = fake
        return fake
    created = api("POST", f"repos/{repo}/issues",
                  {"title": title, "body": body, "labels": labels, "milestone": milestone})
    existing[issue_id] = created
    time.sleep(WRITE_DELAY)
    return created


def ensure_sub_issue(repo: str, parent: dict, child: dict, dry: bool) -> None:
    if dry or parent["number"] < 0 or child["number"] < 0:
        return
    current = api("GET", f"repos/{repo}/issues/{parent['number']}/sub_issues?per_page=100") or []
    if any(s["id"] == child["id"] for s in current):
        return
    api("POST", f"repos/{repo}/issues/{parent['number']}/sub_issues", {"sub_issue_id": child["id"]})
    time.sleep(WRITE_DELAY)


def ensure_blocked_by(repo: str, issue: dict, blocker: dict, dry: bool) -> None:
    if dry or issue["number"] < 0 or blocker["number"] < 0:
        return
    current = api("GET", f"repos/{repo}/issues/{issue['number']}/dependencies/blocked_by?per_page=100") or []
    if any(b["id"] == blocker["id"] for b in current):
        return
    api("POST", f"repos/{repo}/issues/{issue['number']}/dependencies/blocked_by", {"issue_id": blocker["id"]})
    time.sleep(WRITE_DELAY)


def ensure_project(repo: str, title: str, issues: dict[str, dict], dry: bool) -> None:
    owner = repo.split("/")[0]
    print(f"Project board: {title}")
    code, out = gh(["project", "list", "--owner", owner, "--format", "json", "--limit", "100"])
    number = None
    if code == 0 and out.strip():
        for p in json.loads(out).get("projects", []):
            if p.get("title") == title:
                number = p["number"]
    if number is None:
        print("   + creating")
        if dry:
            return
        code, out = gh(["project", "create", "--owner", owner, "--title", title, "--format", "json"])
        if code != 0:
            raise RuntimeError("project create failed (does the token have project scope? run `gh auth refresh -s project`)")
        number = json.loads(out)["number"]
    else:
        print(f"   = #{number}")
    if dry:
        return
    code, out = gh(["project", "item-list", str(number), "--owner", owner, "--format", "json", "--limit", "1000"])
    present = set()
    if code == 0 and out.strip():
        for item in json.loads(out).get("items", []):
            url = (item.get("content") or {}).get("url")
            if url:
                present.add(url)
    for iid, issue in sorted(issues.items()):
        url = issue.get("html_url")
        if not url or url in present:
            continue
        gh(["project", "item-add", str(number), "--owner", owner, "--url", url])
        time.sleep(WRITE_DELAY)
    print("   board columns: rename the Status options to Backlog / Ready / In progress / In review / Done in the project settings")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", required=True, help="OWNER/NAME")
    ap.add_argument("--file", default=str(HERE / "issues.json"))
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--project", action="store_true", help="also create a Projects board and add every issue")
    ap.add_argument("--only", nargs="*", help="limit to these epic IDs, e.g. --only E1 E2")
    args = ap.parse_args()

    data = json.loads(Path(args.file).read_text(encoding="utf-8"))
    project = data["project"]
    epics = data["epics"]
    if args.only:
        epics = [e for e in epics if e["id"] in set(args.only)]

    code, _ = gh(["auth", "status"], ok_codes=(0, 1))
    if code != 0 and not args.dry_run:
        print("gh is not authenticated; run `gh auth login` first")
        return 1

    ensure_labels(args.repo, data["labels"], args.dry_run)
    milestones = ensure_milestones(args.repo, data["milestones"], args.dry_run)
    phase_to_ms = {i + 1: milestones[m["title"]] for i, m in enumerate(data["milestones"])}

    existing = {} if args.dry_run else load_existing_issues(args.repo)
    print(f"Existing issues with IDs: {len(existing)}")

    # 1. epics
    print("Epics")
    for epic in epics:
        create_or_get(args.repo, existing, epic["id"], f"{epic['id']} {epic['title']}",
                      epic_body(epic, project), ["type:epic", f"phase:{epic['phase']}", f"track:{epic.get('track', 'core')}"],
                      phase_to_ms[epic["phase"]], args.dry_run)

    # 2. stories
    print("Stories")
    for epic in epics:
        for story in epic["stories"]:
            create_or_get(args.repo, existing, story["id"], f"{story['id']} {story['title']}",
                          story_body(story, epic, project), story_labels(story, epic),
                          phase_to_ms[epic["phase"]], args.dry_run)

    # 3. sub-issues (epic -> story)
    print("Sub-issues (epic → story)")
    for epic in epics:
        parent = existing[epic["id"]]
        for story in epic["stories"]:
            ensure_sub_issue(args.repo, parent, existing[story["id"]], args.dry_run)
        print(f"   {epic['id']}: {len(epic['stories'])} sub-issues")

    # 4. dependencies (blocked by)
    print("Dependencies (blocked by)")
    missing: list[str] = []
    count = 0
    for epic in epics:
        targets = [(epic["id"], epic.get("depends", []))] + [(s["id"], s.get("depends", [])) for s in epic["stories"]]
        for iid, deps in targets:
            for dep in deps:
                if dep not in existing:
                    missing.append(f"{iid} -> {dep}")
                    continue
                ensure_blocked_by(args.repo, existing[iid], existing[dep], args.dry_run)
                count += 1
    print(f"   {count} relationships")
    if missing:
        print("   unresolved (blocker not created in this run — re-run without --only):")
        for m in missing:
            print(f"      {m}")

    # 5. board
    if args.project:
        ensure_project(args.repo, project["board_title"], existing, args.dry_run)

    if not args.dry_run:
        mapping = {k: {"number": v["number"], "id": v["id"], "url": v.get("html_url", "")} for k, v in existing.items()}
        (HERE / "issues.map.json").write_text(json.dumps(mapping, indent=2), encoding="utf-8")
        print("Wrote issues.map.json")
    print("Done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
