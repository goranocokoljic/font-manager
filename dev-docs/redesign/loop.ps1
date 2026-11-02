# Font Manager UI/UX iteration loop (Windows / PowerShell).
# Usage (from the repo root):  .\dev-docs\redesign\loop.ps1 -Start 1 -Count 10
# Every 5th iteration runs REVIEW.md (the critic) instead of ITERATE.md (the generator);
# both prompts live next to this script. Run it in its own terminal, not in a go-harness lane.
# All work happens on the `redesign` branch: the script creates it from the current branch
# if it does not exist, checks it out, and refuses to run if anything else is checked out.
# Stop any time with Ctrl+C; every finished iteration is already a commit.

param(
    [int]$Start = 1,
    [int]$Count = 10,
    [int]$MaxTurns = 120,
    [string]$Branch = "redesign",
    # Generator passes run on the cheaper model; the review pass (the critic) on the stronger one.
    [string]$IterateModel = "sonnet",
    [string]$ReviewModel = "opus"
)

$ErrorActionPreference = "Stop"
$stopped = $false
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = (git rev-parse --show-toplevel)
Set-Location $repo

# Untracked, machine-local files (.claude/settings.local.json) never block the loop.
function Get-Dirty { git status --porcelain -- . ':(exclude).claude' }

# --- Branch guard -------------------------------------------------------------------
if (Get-Dirty) {
    Write-Host "Working tree is not clean. Commit or stash before running the loop." -ForegroundColor Red
    exit 1
}

$exists = git branch --list $Branch
if (-not $exists) {
    $base = git rev-parse --abbrev-ref HEAD
    Write-Host "Creating branch '$Branch' from '$base'." -ForegroundColor Cyan
    git checkout -b $Branch | Out-Null
} else {
    git checkout $Branch | Out-Null
}

if ((git rev-parse --abbrev-ref HEAD) -ne $Branch) {
    Write-Host "Could not check out '$Branch'. Stopping." -ForegroundColor Red
    exit 1
}

# --- Loop -----------------------------------------------------------------------------
for ($i = $Start; $i -lt $Start + $Count; $i++) {
    # Re-check every iteration: an iteration must never wander off the branch.
    if ((git rev-parse --abbrev-ref HEAD) -ne $Branch) {
        Write-Host "Not on '$Branch' any more — stopping." -ForegroundColor Red
        $stopped = $true
        break
    }

    $isReview = ($i % 5 -eq 0)
    $file = Join-Path $here ($(if ($isReview) { "REVIEW.md" } else { "ITERATE.md" }))
    $kind = if ($isReview) { "review" } else { "iteration" }
    $prompt = "This is $kind $i. You are on branch '$Branch'.`n`n" + (Get-Content $file -Raw)

    Write-Host ""
    Write-Host "=== $kind $i ($file) ===" -ForegroundColor Cyan

    # Non-interactive runs need to execute pnpm, node, Playwright and git without a human
    # to approve each call; acceptEdits only covers file edits (iteration 1 proved it).
    # This is the reason the loop is confined to its own branch.
    $model = if ($isReview) { $ReviewModel } else { $IterateModel }
    # --max-turns is the hard cap on a runaway pass; there is no wall-clock cap because
    # Start-Process cannot stream Claude Code's output and a .cmd shim resolves unreliably.
    claude -p $prompt --model $model --dangerously-skip-permissions --max-turns $MaxTurns
    $code = $LASTEXITCODE

    if ($code -ne 0) {
        Write-Host "$kind $i exited with code $code — stopping." -ForegroundColor Red
        $stopped = $true
        break
    }

    # Sanity check: the iteration must have left a commit behind, and the tree must be clean.
    $last = git log -1 --pretty=%s
    if ($last -notmatch "^(iter|review)\($i\)") {
        Write-Host "No commit found for $kind $i (last: '$last') — stopping so you can look." -ForegroundColor Yellow
        $stopped = $true
        break
    }
    if (Get-Dirty) {
        Write-Host "$kind $i left uncommitted changes — stopping so you can look." -ForegroundColor Yellow
        $stopped = $true
        break
    }
}

Write-Host ""
if ($stopped) {
    Write-Host "Stopped early — see the message above. Fix, then re-run with -Start <that iteration>." -ForegroundColor Yellow
    exit 1
}
Write-Host "Done. Read dev-docs/redesign/ITERATIONS.md, look at .harness/redesign/shots/, and 'git log --oneline main..$Branch'." -ForegroundColor Green
