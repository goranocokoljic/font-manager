#!/usr/bin/env pwsh
# go-harness.ps1 — drive Font Manager stories from GitHub issue to green PR,
# one lane at a time, with a pinned status panel at the bottom of the terminal.
#
# Adapted from tr-harness (toprope / profile). What stays: a fresh headless
# `claude` process per stage, the live panel, authoritative billed cost,
# riding out usage limits, failure quarantine with a circuit breaker, the
# convergence guard, and the human-gated review-KB graduation.
# What is new for this repo (see dev-docs/HARNESS-ADAPTATION.md):
#   - LANES. A lane (core | ui) owns a set of fm-meta tracks and works in its
#     own git worktree. Run one harness process per lane, in two terminals.
#   - THE PICKER chooses the story: Definition of Ready from GitHub
#     (blocked_by dependencies, labels, assignee), lane ordering, overlap with
#     the other lane's open PR (scripts/harness/pick.mjs).
#   - STAGES. A story runs as build -> CI -> review -> CI -> finalize, each
#     stage one `claude` run of /go-cycle; later stages resume the build
#     session (`claude --resume`), so the implementation intent survives.
#   - CI IS THE JUDGE. Between stages the harness itself waits for the
#     Windows/macOS/Linux matrix (scripts/harness/ci-wait.mjs) — no tokens are
#     spent waiting. A red OS loops back to a ci-fix stage with the job log;
#     3 failures -> harness:blocked. Numeric criteria add the bench job.
#   - THE MERGE GATE. `merge: human` leaves a green, reviewed, ready PR and
#     never merges. `merge: auto` merges it when CI is green on a branch that
#     is up to date with main, the review has no open must
#     and no existing fm-api type changed; platform-track PRs and contract
#     changes wait for a human while the lane goes on (merge-check.mjs).
#     Stop conditions set harness:blocked with a comment.
#   - BUDGETS by size (S 1x, M 2.5x, L 5x the base in go-harness.config.json).
#     Off by default (spend is still shown and logged); -Budget or
#     "budget_enforced": true stops a story that goes over.
#   - TELEMETRY in .harness/ (git-ignored): per-stage runs, review cycles,
#     one record per story, and fm-run-summary.md. Epics get a summary
#     comment when a story closes (scripts/harness/epic-sync.mjs).
#   - PURECONTEXT. Every stage calls the PureContext MCP index; the panel
#     shows the call count and the loop guard warns when a story used none.
#
# Usage:
#   ./go-harness.ps1 -DryRun                       # which story each lane would pick and why; touches nothing
#   ./go-harness.ps1 -Lane core                    # work the core lane until nothing is ready
#   ./go-harness.ps1 -Lane ui -MaxStories 1        # one ui story, then stop
#   ./go-harness.ps1 -Lane core -MaxStories 10     # night run: 10 stories; a blocked story is skipped, the lane goes on
#   ./go-harness.ps1 -Lane core -MaxStories 10 -StopOnBlock   # ...or stop at the first blocked one
#   ./go-harness.ps1 -Lane core -Graduate          # ask the KB graduation question at the end (interactive)
#   ./go-harness.ps1 -Lane core -Issues 42         # only these issues (still Definition-of-Ready checked)
#   ./go-harness.ps1 -Lane core -NoWorktree        # run in THIS checkout (to watch the first story)
#   ./go-harness.ps1 -Lane core -Model fable       # default opus
#   ./go-harness.ps1 -Lane core -Budget            # stop a story that goes over its size budget
#   ./go-harness.ps1 -GraduateOnly                 # just the KB graduation review
#   ./go-harness.ps1 -EpicSync                     # just post epic summaries / close finished epics
#
# Exit code: 0 clean, 1 a story blocked or failed, 2 setup problem.

[CmdletBinding()]
param(
    [ValidateSet('core', 'ui')]
    [string]$Lane,

    # Restrict the lane to these issue numbers (Definition of Ready still applies).
    [int[]]$Issues,

    # Stop after this many stories (0 = until the lane has nothing ready).
    [int]$MaxStories = 0,

    [ValidateSet('sonnet', 'opus', 'fable')]
    [string]$Model = 'opus',

    # Review cycles per story (cycle 1 full review, later cycles verify the fix).
    # 0 = review.max_cycles from go-harness.config.json.
    [ValidateRange(0, 6)]
    [int]$ReviewCycles = 0,

    # Resumes of ONE stage after an incomplete (crashed / no-sentinel) run.
    [int]$MaxResumes = 2,

    [switch]$Panel,
    [switch]$NoPanel,

    # Run in this checkout instead of the lane's worktree.
    [switch]$NoWorktree,

    # Enforce the per-story budget (off by default: a subscription plan pays
    # no per-token cost). Also on with "budget_enforced": true in the config.
    [switch]$Budget,

    [switch]$NoWaitForReset,
    [int]$MaxWaits = 12,

    # This many stories in a row blocked/failed looks systemic -> stop the lane.
    [ValidateRange(1, 100)]
    [int]$MaxConsecutiveFails = 3,

    # Stop the lane at the first story that ends blocked or failed (night runs).
    [switch]$StopOnBlock,

    # Idle while the other lane still works: end at once instead of waiting
    # (idle.poll_min / idle.max_hours in go-harness.config.json).
    [switch]$NoIdleWait,

    [switch]$NoLoopGuard,
    # Ask the interactive KB graduation question when the lane ends. Off by
    # default, so an unattended run never waits for a key (-GraduateOnly later).
    [switch]$Graduate,
    [switch]$NoGraduate,
    [switch]$GraduateOnly,
    [switch]$DryRun,
    [switch]$EpicSync
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

$OK      = 'GOCYCLE_OK'
$FAIL    = 'GOCYCLE_FAIL'
$BLOCKED = 'GOCYCLE_BLOCKED'

$HarnessDir   = Join-Path $PSScriptRoot '.harness'
$logDir       = Join-Path $HarnessDir 'logs'
$stateDir     = Join-Path $HarnessDir 'state'
$tasksJsonl   = Join-Path $HarnessDir 'tasks.jsonl'          # one record per claude stage run
$cyclesJsonl  = Join-Path $HarnessDir 'review-cycles.jsonl'  # one record per review cycle
$storiesJsonl = Join-Path $HarnessDir 'stories.jsonl'        # one record per finished story
$summaryMd    = Join-Path $HarnessDir 'fm-run-summary.md'
foreach ($d in $HarnessDir, $logDir, $stateDir) { New-Item -ItemType Directory -Force -Path $d | Out-Null }

$Cfg = Get-Content (Join-Path $PSScriptRoot 'go-harness.config.json') -Raw | ConvertFrom-Json
if ($ReviewCycles -eq 0) { $ReviewCycles = [int]($Cfg.review.max_cycles ?? 3) }
$CiBudget  = [int]($Cfg.ci.retry_budget ?? 3)
$BaseUsd   = [double]($Cfg.budget_base_usd ?? 15)
$BudgetOn  = $Budget.IsPresent -or [bool]($Cfg.budget_enforced ?? $false)
$SizeFactor = @{ S = 1.0; M = 2.5; L = 5.0 }
$WaitForReset = -not $NoWaitForReset
$maxAttempts  = $MaxResumes + 1

# --------------------------------------------------------------------------
# Panel state
# --------------------------------------------------------------------------
$ESC       = [char]27
$PHASES    = @('read', 'branch', 'implement', 'preflight', 'pr', 'ci', 'review', 'finalize', 'merge')
$PanelRows = 8
$script:PanelMode = (-not $NoPanel) -and ($Panel -or [bool]$env:WT_SESSION) -and
    (& { try { [Console]::WindowHeight -gt ($PanelRows + 4) } catch { $false } })

$script:RunStart    = Get-Date
$script:Done        = [ordered]@{}   # story id -> 'done'|'ready'|'blocked'|'failed'
$script:Story       = $null          # current story state (hashtable)
$script:Stage       = ''
$script:StageStart  = $null
$script:Attempt     = 0
$script:Phase       = $null
$script:PhaseDetail = ''
$script:NowDetail   = ''
$script:ModelArg    = $Model
$script:RunModel    = ''
$script:RunState    = 'running'
$script:RunCost     = 0.0
$script:LastCost    = 0.0
$script:CurCtx      = 0.0
$script:LimitHit    = $false
$script:LimitReason = ''
$script:SessionId   = ''
$script:PcCalls     = 0              # PureContext tool calls in the current claude run
$script:ConsecFails = 0
$script:HadFailures = $false
$script:StopReason  = $null   # set by the merge gate when main is red
$script:Work        = $PSScriptRoot  # the checkout claude runs in (lane worktree)

$script:RunPhases = [ordered]@{}; $script:RunCycles = @(); $script:CurPhase = $null
$script:CurCycle = $null; $script:CurSub = $null; $script:RunStartTs = $null; $script:RunEndTs = $null
$script:RunCriteria = $null

$C = @{
    reset = '0'; bold = '1'; dim = '2'
    red = '31'; green = '32'; yellow = '33'; cyan = '36'; gray = '90'
    boldcyan = '1;36'; boldgreen = '1;32'; boldred = '1;31'
}

# --------------------------------------------------------------------------
# Cost & context. We never reconstruct cost from per-turn usage (cache tokens
# are re-reported cumulatively; summing them overstated cost 2-70x in
# tr-harness). The authoritative total_cost_usd comes from each process's
# result line; per-turn usage only drives the live context gauge.
# --------------------------------------------------------------------------
function Update-Ctx {
    param($Usage)
    if ($null -eq $Usage) { return }
    $cc = $Usage.cache_creation
    $w = if ($cc) { [double]$cc.ephemeral_5m_input_tokens + [double]$cc.ephemeral_1h_input_tokens }
         else { [double]$Usage.cache_creation_input_tokens }
    $script:CurCtx = [double]$Usage.input_tokens + [double]$Usage.cache_read_input_tokens + $w
}

# --------------------------------------------------------------------------
# Usage / session limits: a limit stops the run without a sentinel. Detect it,
# sleep until the reset, re-dispatch the same attempt (it is not counted).
# --------------------------------------------------------------------------
function Test-UsageLimit {
    param([string]$ResultText, $ApiErrorStatus)
    if ($null -ne $ApiErrorStatus -and "$ApiErrorStatus".Trim() -ne '' -and "$ApiErrorStatus" -ne 'null') { return $true }
    return ($ResultText -match '(?i)usage limit|rate[ _-]?limit|too many requests|overloaded|quota (?:exceeded|reached)|limit (?:reached|will reset)|reset[s]? at')
}

function Test-LimitLine {
    param([string]$Line)
    if ([string]::IsNullOrWhiteSpace($Line)) { return $false }
    return ($Line -match "(?i)hit your (?:session|usage|weekly|5-?hour|account) limit|(?:usage|session) limit reached|claude usage limit|approaching your (?:session|usage) limit|limit · reset")
}

function Get-ResetTime {
    param([string]$Reason)
    $m = [regex]::Match([string]$Reason, '(?i)reset[s]?\s+(?:at\s+)?(\d{1,2})(?::(\d{2}))?\s*(am|pm)?')
    if (-not $m.Success) { return $null }
    $h = [int]$m.Groups[1].Value
    $min = if ($m.Groups[2].Success) { [int]$m.Groups[2].Value } else { 0 }
    $ap = $m.Groups[3].Value.ToLower()
    if ($ap -eq 'pm' -and $h -lt 12) { $h += 12 } elseif ($ap -eq 'am' -and $h -eq 12) { $h = 0 }
    if ($h -gt 23 -or $min -gt 59) { return $null }
    $t = (Get-Date -Hour $h -Minute $min -Second 0)
    if ($t -le (Get-Date)) { $t = $t.AddDays(1) }
    return $t
}

function Wait-UntilReset {
    param([string]$Reason, [int]$FallbackMinutes = 30, [int]$BufferSeconds = 120)
    $target = Get-ResetTime -Reason $Reason
    if ($null -eq $target) { $target = (Get-Date).AddMinutes($FallbackMinutes); $label = 'no reset time in message; retrying at' }
    else { $target = $target.AddSeconds($BufferSeconds); $label = 'limit hit; resuming at' }
    $lastLog = (Get-Date).AddMinutes(-10)
    while ((Get-Date) -lt $target) {
        $left = $target - (Get-Date)
        if (((Get-Date) - $lastLog).TotalSeconds -ge 300 -or $left.TotalSeconds -le 65) {
            Write-Log ("    [wait] $label $($target.ToString('ddd HH:mm')) — {0}h{1:00}m left" -f [int]$left.TotalHours, $left.Minutes) 'Yellow'
            $lastLog = Get-Date
        }
        Start-Sleep -Seconds ([Math]::Min(60, [Math]::Max(5, [int]$left.TotalSeconds)))
    }
}

# --------------------------------------------------------------------------
# Analytics capture (per claude run): phase timing, token VOLUME (output + new
# input only), review cycles from GOCYCLE_METRIC lines, PureContext calls.
# est_cost_usd = the billed total apportioned by output-token share: an estimate.
# --------------------------------------------------------------------------
function Reset-Analytics {
    $script:RunPhases = [ordered]@{}; $script:RunCycles = @(); $script:CurPhase = $null
    $script:CurCycle = $null; $script:CurSub = $null; $script:RunStartTs = Get-Date; $script:RunEndTs = $null
    $script:PcCalls = 0; $script:RunCriteria = $null
}

function Open-Phase {
    param([string]$Name)
    if ($script:CurPhase -and -not $script:CurPhase.end) { $script:CurPhase.end = Get-Date }
    if (-not $script:RunPhases.Contains($Name)) {
        $script:RunPhases[$Name] = [ordered]@{ phase = $Name; start = (Get-Date); end = $null; out = 0.0; in = 0.0; turns = 0; tools = 0; pc = 0; peak = 0.0 }
    }
    $script:CurPhase = $script:RunPhases[$Name]
}

function Start-ReviewCycle {
    param([int]$N, [int]$Max)
    $rec = [ordered]@{
        review_cycle = $N; max_cycles = $Max
        review_start = (Get-Date); review_end = $null; fix_start = $null; fix_end = $null
        must = 0; should = 0; rejected = 0; resolved = 0; raw = 0; findings_source = 'none'; by_lens = $null
        fixed = $null; disputed = $null
        out = 0.0; in = 0.0; review_out = 0.0; fix_out = 0.0; turns = 0; tools = 0
    }
    $script:RunCycles += ,$rec
    $script:CurCycle = $rec
    $script:CurSub = 'review'
}

function Add-TurnTokens {
    param($Usage, [bool]$IsMain)
    if ($null -eq $Usage) { return }
    $out = [double]$Usage.output_tokens; $in = [double]$Usage.input_tokens
    if ($script:CurPhase) {
        $script:CurPhase.out += $out; $script:CurPhase.in += $in; $script:CurPhase.turns++
        if ($IsMain -and $script:CurCtx -gt $script:CurPhase.peak) { $script:CurPhase.peak = $script:CurCtx }
    }
    if ($script:CurCycle -and $script:CurPhase -and $script:CurPhase.phase -eq 'review') {
        $script:CurCycle.out += $out; $script:CurCycle.in += $in; $script:CurCycle.turns++
        if ($script:CurSub -eq 'review') { $script:CurCycle.review_out += $out }
        elseif ($script:CurSub -eq 'fix') { $script:CurCycle.fix_out += $out }
    }
}

function Read-MetricMarker {
    param([string]$Raw)
    $m = [regex]::Match($Raw, '^\s*GOCYCLE_METRIC:\s*(\w+)\s*(?:\|\s*(.*))?$')
    if (-not $m.Success) { return $false }
    $kv = @{}
    foreach ($pair in [regex]::Matches($m.Groups[2].Value, '(\w+)\s*=\s*(\d+)')) { $kv[$pair.Groups[1].Value] = [int]$pair.Groups[2].Value }
    switch ($m.Groups[1].Value) {
        'review_done' {
            if ($script:CurCycle) {
                foreach ($k in 'must', 'should', 'rejected', 'resolved', 'raw') { if ($kv.ContainsKey($k)) { $script:CurCycle[$k] = $kv[$k] } }
                $script:CurCycle.findings_source = 'marker'
                $script:CurCycle.review_end = Get-Date
                $script:CurSub = 'between'
                $script:PhaseDetail = "cycle $($script:CurCycle.review_cycle): must $($script:CurCycle.must), should $($script:CurCycle.should), rejected $($script:CurCycle.rejected)"
            }
        }
        'fix_start' { if ($script:CurCycle) { $script:CurCycle.fix_start = Get-Date; $script:CurSub = 'fix' } }
        'fix_done'  { if ($script:CurCycle) { $script:CurCycle.fix_end = Get-Date; $script:CurSub = 'done' } }
        'dispositions' {
            if ($script:CurCycle) { foreach ($k in 'fixed', 'disputed') { $script:CurCycle[$k] = if ($kv.ContainsKey($k)) { $kv[$k] } else { 0 } } }
        }
        'criteria' { $script:RunCriteria = [ordered]@{ ticked = [int]($kv.ticked ?? 0); pending = [int]($kv.pending ?? 0) } }
    }
    return $true
}

function Get-Sec { param($A, $B) if ($A -and $B) { [math]::Round((($B) - ($A)).TotalSeconds, 1) } else { $null } }

# One record per claude stage run (tasks.jsonl) + its review cycles (review-cycles.jsonl).
function Write-Analytics {
    param([string]$Outcome, [double]$BilledUsd)
    if ($script:CurPhase -and -not $script:CurPhase.end) { $script:CurPhase.end = Get-Date }
    $script:RunEndTs = Get-Date
    $s = $script:Story
    $ts = (Get-Date).ToString('o')
    $totalOut = 0.0; foreach ($p in $script:RunPhases.Values) { $totalOut += $p.out }
    $apportion = { param($o) if ($BilledUsd -gt 0 -and $totalOut -gt 0) { [math]::Round($BilledUsd * $o / $totalOut, 4) } else { 0.0 } }

    # Read each cycle's review file: always for the per-lens split (the dashboard's
    # verifier-by-lens view), and for the counts when no review_done marker came.
    foreach ($c in $script:RunCycles) {
        $rf = Join-Path $script:Work ('.harness/reviews/issue-{0}-review-{1}.md' -f $s.issue, $c.review_cycle)
        if (-not (Test-Path $rf)) { continue }
        $r = Invoke-Node 'review-counts.mjs' @($rf)
        if ($r.Code -ne 0) { continue }
        try {
            $counts = $r.Out | ConvertFrom-Json -AsHashtable
            $c.by_lens = $counts.by_lens
            if ($c.findings_source -ne 'marker') {
                foreach ($k in 'must', 'should', 'rejected', 'resolved', 'raw') { $c[$k] = [int]$counts[$k] }
                $c.findings_source = 'review-file'
            }
        } catch { }
    }

    $cycleRecs = @(foreach ($c in $script:RunCycles) {
        $end = if ($c.fix_end) { $c.fix_end } else { $c.review_end }
        [ordered]@{
            issue = $s.issue; id = $s.id; ts = $ts; review_cycle = $c.review_cycle; max_cycles = $c.max_cycles
            mode = $(if ($c.review_cycle -eq 1) { 'full' } else { 'verify' })
            findings = [ordered]@{ must = $c.must; should = $c.should; rejected = $c.rejected; resolved = $c.resolved; raw = $c.raw }
            findings_source = $c.findings_source
            by_lens = $c.by_lens
            dispositions = $(if ($null -ne $c.fixed -or $null -ne $c.disputed) { [ordered]@{ fixed = [int]($c.fixed ?? 0); disputed = [int]($c.disputed ?? 0) } } else { $null })
            review_sec = Get-Sec $c.review_start $c.review_end; fix_sec = Get-Sec $c.fix_start $c.fix_end
            cycle_sec = Get-Sec $c.review_start $end
            out_tokens = [long]$c.out; in_tokens = [long]$c.in; review_out_tokens = [long]$c.review_out; fix_out_tokens = [long]$c.fix_out
            turns = $c.turns; tool_calls = $c.tools; est_cost_usd = & $apportion $c.out
        }
    })
    foreach ($r in $cycleRecs) { Add-Content -Path $cyclesJsonl -Value ($r | ConvertTo-Json -Depth 6 -Compress) }

    $task = [ordered]@{
        issue = $s.issue; id = $s.id; lane = $Lane; stage = $script:Stage; attempt = $script:Attempt; ts = $ts
        outcome = $Outcome; model_asked = $script:ModelArg; model_ran = $script:RunModel; session_id = $script:SessionId
        total_sec = Get-Sec $script:RunStartTs $script:RunEndTs; billed_cost_usd = [math]::Round($BilledUsd, 4)
        purecontext_calls = $script:PcCalls
        phases = @(foreach ($p in $script:RunPhases.Values) {
            [ordered]@{ phase = $p.phase; duration_sec = Get-Sec $p.start $p.end; out_tokens = [long]$p.out; in_tokens = [long]$p.in
                        turns = $p.turns; tool_calls = $p.tools; purecontext_calls = $p.pc; peak_ctx_tokens = [long]$p.peak; est_cost_usd = & $apportion $p.out }
        })
        review_cycles = $cycleRecs
        criteria = $script:RunCriteria
    }
    Add-Content -Path $tasksJsonl -Value ($task | ConvertTo-Json -Depth 8 -Compress)

    # Fold into the story roll-up.
    $s.billed_usd += $BilledUsd
    $s.purecontext_calls += $script:PcCalls
    foreach ($c in $cycleRecs) {
        $s.review_rounds++
        if ($c.review_cycle -eq 1) { $s.must_total += $c.findings.must; $s.should_total += $c.findings.should }
        $s.rejected_total += $c.findings.rejected
        if ($c.dispositions) { $s.must_fixed += $c.dispositions.fixed }
    }
}

# --------------------------------------------------------------------------
# Terminal + panel
# --------------------------------------------------------------------------
function Set-ScrollRegion { param([int]$Top, [int]$Bottom) [Console]::Write("$ESC[$Top;${Bottom}r") }
function Reset-ScrollRegion { [Console]::Write("$ESC[r") }

function Format-Line {
    param([object[]]$Segs, [int]$Width)
    $sb = ''; $vis = 0
    foreach ($s in $Segs) {
        if ($vis -ge $Width) { break }
        $t = [string]$s.t
        if ($vis + $t.Length -gt $Width) { $t = $t.Substring(0, [Math]::Max(0, $Width - $vis)) }
        if ($t.Length -gt 0) { $sb += $(if ($s.c) { "$ESC[$($s.c)m$t$ESC[0m" } else { $t }); $vis += $t.Length }
    }
    return $sb
}

function Seg { param([string]$t, [string]$c = '') return @{ t = $t; c = $c } }

function Get-PanelLines {
    param([int]$Width)
    $rule = @(Seg ('─' * $Width) $C.gray)
    $el = (Get-Date) - $script:RunStart
    $line1 = @(
        Seg " go-harness [$Lane]  " $C.bold
        Seg ('started {0}   ' -f $script:RunStart.ToString('HH:mm:ss')) $C.gray
        Seg ('elapsed {0:00}:{1:00}:{2:00}   ' -f [int]$el.TotalHours, $el.Minutes, $el.Seconds) ''
        Seg ('$' + ('{0:N2}' -f $script:RunCost) + ' billed') $(if ($script:RunState -eq 'stopped') { $C.boldred } else { $C.boldgreen })
        Seg "   merge: $($Cfg.merge)" $C.gray
    )
    $fam = if ($script:RunModel -match 'opus') { 'opus' } elseif ($script:RunModel -match 'fable') { 'fable' } elseif ($script:RunModel -match 'sonnet') { 'sonnet' } elseif ($script:RunModel -match 'haiku') { 'haiku' } else { $script:RunModel }
    if (-not $fam) { $line1 += Seg ("   model {0}?" -f $script:ModelArg) $C.gray }
    elseif ($fam -eq $script:ModelArg) { $line1 += Seg ("   model {0}" -f $fam) $C.cyan }
    else { $line1 += Seg ("   model {0} (asked {1}!)" -f $fam, $script:ModelArg) $C.boldred }
    if ($script:RunState -eq 'stopped') { $line1 += Seg '   STOPPED' $C.boldred }
    elseif ($script:RunState -eq 'paused') { $line1 += Seg '   ⏸ LIMIT — resuming after reset' $C.yellow }
    elseif ($script:RunState -eq 'idle') { $line1 += Seg '   IDLE — waiting for the other lane' $C.yellow }

    $q = @(Seg ' stories: ' $C.gray)
    foreach ($k in $script:Done.Keys) {
        $st = $script:Done[$k]
        $q += switch ($st) {
            'done'    { Seg "✓$k  " $C.green }
            'ready'   { Seg "✓$k(PR)  " $C.green }
            'blocked' { Seg "⊘$k  " $C.yellow }
            default   { Seg "✗$k  " $C.red }
        }
    }
    if ($script:Story) { $q += Seg "▶$($script:Story.id)" $C.boldcyan }
    if (-not $script:Done.Count -and -not $script:Story) { $q += Seg '(picking…)' $C.gray }

    if ($script:Story) {
        $s = $script:Story
        $inFor = if ($script:StageStart) { $d = (Get-Date) - $script:StageStart; '{0}m {1:00}s' -f [int]$d.TotalMinutes, $d.Seconds } else { '' }
        $bcol = if ($s.billed_usd -gt $s.budget_usd) { $C.boldred } elseif ($s.billed_usd -gt 0.8 * $s.budget_usd) { $C.yellow } else { '' }
        $ctxK = $script:CurCtx / 1e3
        $line3 = @(
            Seg " ▶ #$($s.issue) $($s.id)  " $C.boldcyan
            Seg "[$($s.size) $($s.track)]  " $C.cyan
            Seg "stage $($script:Stage) $inFor" ''
            Seg ("   attempt {0}/{1}" -f $script:Attempt, $maxAttempts) $C.gray
            Seg ('   $' + ('{0:N2}' -f $s.billed_usd) + ' / ' + ('{0:N0}' -f $s.budget_usd)) $bcol
            Seg ("   ci {0}/{1}" -f $s.ci_attempts, $CiBudget) $(if ($s.ci_attempts -gt 0) { $C.yellow } else { $C.gray })
            Seg ("   pc {0}" -f ($s.purecontext_calls + $script:PcCalls)) $(if (($s.purecontext_calls + $script:PcCalls) -eq 0) { $C.yellow } else { $C.gray })
            Seg ('   ctx ~{0:N0}K' -f $ctxK) $(if ($ctxK -gt 300) { $C.boldred } elseif ($ctxK -gt 180) { $C.yellow } else { $C.gray })
        )
    } else { $line3 = @(Seg ' (no story running)' $C.gray) }

    $curIdx = if ($script:Phase) { [Array]::IndexOf($PHASES, $script:Phase) } else { -1 }
    $track = @(Seg '   phase  ' $C.gray)
    for ($i = 0; $i -lt $PHASES.Count; $i++) {
        if ($i -gt 0) { $track += Seg ' ─ ' $C.gray }
        $track += if ($i -lt $curIdx) { Seg $PHASES[$i] $C.green } elseif ($i -eq $curIdx) { Seg ('▶' + $PHASES[$i]) $C.boldcyan } else { Seg $PHASES[$i] $C.gray }
    }
    $now = if ($script:Phase) { $script:Phase + $(if ($script:PhaseDetail) { " ($($script:PhaseDetail))" }) + $(if ($script:NowDetail) { "   $($script:NowDetail)" }) } else { '(waiting for first phase marker…)' }
    return @((Format-Line $rule $Width), (Format-Line $line1 $Width), (Format-Line $q $Width), (Format-Line $rule $Width),
             (Format-Line $line3 $Width), (Format-Line $track $Width), (Format-Line @(Seg '   now    ' $C.gray; Seg $now '') $Width), (Format-Line $rule $Width))
}

function Draw-Panel {
    if (-not $script:PanelMode) { return }
    $h = [Console]::WindowHeight; $w = [Console]::WindowWidth
    $top = $h - $PanelRows + 1
    $lines = Get-PanelLines -Width ($w - 1)
    $out = "$ESC`7"
    for ($i = 0; $i -lt $PanelRows; $i++) { $out += "$ESC[$($top + $i);1H$ESC[2K$(if ($i -lt $lines.Count) { $lines[$i] })" }
    [Console]::Write($out + "$ESC`8")
}

function Initialize-Panel {
    if (-not $script:PanelMode) { return }
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
    [Console]::Write("$ESC[2J$ESC[H")
    Set-ScrollRegion 1 ([Console]::WindowHeight - $PanelRows)
    [Console]::Write("$ESC[1;1H")
    Draw-Panel
}

function Close-Panel {
    if (-not $script:PanelMode) { return }
    Draw-Panel; Reset-ScrollRegion
    [Console]::Write("$ESC[$([Console]::WindowHeight);1H`n")
}

function Write-Log {
    param([string]$Text, [string]$Color = 'Gray')
    Write-Host $Text -ForegroundColor $Color
    Draw-Panel
}

# --------------------------------------------------------------------------
# Node helpers. Run a scripts/harness/*.mjs from THIS checkout (the current
# harness version) with the lane worktree as working directory, so `gh`
# resolves the repo and file checks see the story branch. Start-Process keeps
# the panel alive while a long call (ci-wait) runs.
# --------------------------------------------------------------------------
function ConvertTo-ArgString {
    param([string[]]$Arguments)
    ($Arguments | ForEach-Object { '"' + ([string]$_ -replace '(\\*)"', '$1$1\"' -replace '(\\+)$', '$1$1') + '"' }) -join ' '
}

function Invoke-Node {
    param([string]$Script, [string[]]$Arguments = @(), [switch]$Live)
    $tmpOut = [IO.Path]::GetTempFileName(); $tmpErr = [IO.Path]::GetTempFileName()
    try {
        $argStr = ConvertTo-ArgString (@((Join-Path $PSScriptRoot "scripts/harness/$Script")) + $Arguments)
        $p = Start-Process -FilePath 'node' -ArgumentList $argStr -WorkingDirectory $script:Work -NoNewWindow -PassThru `
                           -RedirectStandardOutput $tmpOut -RedirectStandardError $tmpErr
        $null = $p.Handle
        while (-not $p.WaitForExit(5000)) { if ($Live) { Draw-Panel } }
        return @{ Code = $p.ExitCode; Out = (Get-Content $tmpOut -Raw) ?? ''; Err = (Get-Content $tmpErr -Raw) ?? '' }
    } finally { Remove-Item $tmpOut, $tmpErr -Force -ErrorAction SilentlyContinue }
}

function Get-NodeJson {
    param([string]$Script, [string[]]$Arguments = @())
    $r = Invoke-Node $Script $Arguments
    if ($r.Code -ne 0 -or -not $r.Out.Trim()) { throw "$Script failed (exit $($r.Code)): $($r.Err.Trim())" }
    return ($r.Out | ConvertFrom-Json -Depth 20)
}

# --------------------------------------------------------------------------
# One headless claude run of /go-cycle (one stage), streamed and parsed.
# --------------------------------------------------------------------------
function Invoke-ClaudeRun {
    param([string]$Prompt, [string]$LogPath, [string]$ResumeId = '')
    $script:FinalText = ''; $script:CliExit = 0; $script:LastCost = 0.0
    $script:LimitHit = $false; $script:LimitReason = ''; $script:LimitSeen = $false; $script:LimitMsg = ''; $script:ApiErr = $null
    $script:SessionId = ''
    Reset-Analytics
    $claudeArgs = @('-p', $Prompt, '--model', $script:ModelArg, '--output-format', 'stream-json', '--verbose', '--dangerously-skip-permissions')
    if ($ResumeId) { $claudeArgs += @('--resume', $ResumeId) }

    $writer = [System.IO.StreamWriter]::new($LogPath, $false); $writer.AutoFlush = $true
    Push-Location $script:Work
    try {
        claude @claudeArgs | ForEach-Object {
            $line = [string]$_
            $writer.WriteLine($line)
            if (-not $script:LimitSeen -and (Test-LimitLine $line)) {
                $script:LimitSeen = $true
                $t = $line.Trim()
                $script:LimitMsg = if ($t.StartsWith('{')) { 'session/usage limit reached' } else { $t.Substring(0, [Math]::Min(140, $t.Length)) }
            }
            $obj = $null
            try { $obj = $line | ConvertFrom-Json -ErrorAction Stop } catch { }
            if ($null -eq $obj) { if ($line.Trim()) { Write-Log "    $line" 'DarkGray' }; return }
            switch ($obj.type) {
                'system' {
                    if ($obj.subtype -eq 'init') {
                        if ($obj.model) { $script:RunModel = [string]$obj.model }
                        if ($obj.session_id) { $script:SessionId = [string]$obj.session_id }
                        $pcTools = @($obj.tools | Where-Object { "$_" -like 'mcp__purecontext*' }).Count
                        Write-Log "    * session $($script:SessionId) ($($script:RunModel)), PureContext tools: $pcTools" $(if ($pcTools) { 'DarkGray' } else { 'Yellow' })
                    }
                }
                'assistant' {
                    $isMain = -not $obj.parent_tool_use_id
                    Update-Ctx -Usage $obj.message.usage
                    Add-TurnTokens -Usage $obj.message.usage -IsMain $isMain
                    if ($obj.message.model -and $isMain) { $script:RunModel = [string]$obj.message.model }
                    foreach ($block in $obj.message.content) {
                        if ($block.type -eq 'text' -and $block.text.Trim()) {
                            foreach ($raw in ($block.text -split "`r?`n")) {
                                $raw = $raw.Trim() -replace '^[`*_]+|[`*_]+$', ''
                                if (Read-MetricMarker $raw) { Write-Log "    ◆ $raw" 'DarkMagenta'; continue }
                                $m = [regex]::Match($raw, '^\s*GOCYCLE_PHASE:\s*(.+?)\s*$')
                                if ($m.Success) {
                                    $parts = $m.Groups[1].Value -split '\|', 2
                                    $name = $parts[0].Trim()
                                    if ($PHASES -contains $name) {
                                        $script:Phase = $name
                                        $script:PhaseDetail = if ($parts.Count -gt 1) { $parts[1].Trim() } else { '' }
                                        Open-Phase $name
                                        $cm = [regex]::Match($script:PhaseDetail, 'cycle\s*(\d+)\s*/\s*(\d+)')
                                        if ($name -eq 'review' -and $cm.Success) { Start-ReviewCycle ([int]$cm.Groups[1].Value) ([int]$cm.Groups[2].Value) }
                                        Write-Log "    ▸ phase: $name $($script:PhaseDetail)" 'Magenta'
                                    }
                                }
                            }
                            $t = ($block.text.Trim() -replace '\s+', ' ')
                            if ($t -notmatch '^[`*_]*GOCYCLE_PHASE:') { Write-Log ("    " + $(if ($t.Length -gt 160) { $t.Substring(0, 160) + '...' } else { $t })) 'Gray' }
                        }
                        elseif ($block.type -eq 'tool_use') {
                            $isPc = "$($block.name)" -like 'mcp__purecontext*'
                            if ($isPc) { $script:PcCalls++; if ($script:CurPhase) { $script:CurPhase.pc++ } }
                            if ($script:CurPhase) { $script:CurPhase.tools++ }
                            if ($script:CurCycle -and $script:CurPhase -and $script:CurPhase.phase -eq 'review') { $script:CurCycle.tools++ }
                            $d = if ($block.input.command) { ": $($block.input.command)" } elseif ($block.input.file_path) { ": $($block.input.file_path)" } elseif ($block.input.pattern) { ": $($block.input.pattern)" } elseif ($block.input.task) { ": $($block.input.task)" } else { '' }
                            $name = ($block.name -replace '^mcp__purecontext-mcp__', 'pc:')
                            $script:NowDetail = "$name$d".Trim(); if ($script:NowDetail.Length -gt 80) { $script:NowDetail = $script:NowDetail.Substring(0, 80) + '...' }
                            if ($d.Length -gt 120) { $d = $d.Substring(0, 120) + '...' }
                            Write-Log "    -> $name$d" $(if ($isPc) { 'DarkCyan' } else { 'Cyan' })
                        }
                    }
                }
                'result' {
                    if ($null -ne $obj.result) { $script:FinalText = [string]$obj.result }
                    if ($null -ne $obj.total_cost_usd) { $script:LastCost = [double]$obj.total_cost_usd }
                    if ($null -ne $obj.api_error_status) { $script:ApiErr = $obj.api_error_status }
                    if ($obj.session_id) { $script:SessionId = [string]$obj.session_id }
                }
            }
        }
        $script:CliExit = $LASTEXITCODE
    }
    finally { Pop-Location; $writer.Dispose() }
    $script:LimitHit = $script:LimitSeen -or (Test-UsageLimit -ResultText $script:FinalText -ApiErrorStatus $script:ApiErr)
    $script:LimitReason = if ($script:LimitMsg) { $script:LimitMsg } elseif ($script:LimitHit) { 'usage/API limit' } else { '' }
    return $script:FinalText
}

# Run one stage to a verdict: 'ok' | 'blocked' | 'failed' | 'incomplete'.
# Resumes an incomplete run up to -MaxResumes times; rides out usage limits.
function Invoke-Stage {
    param([string]$Stage, [string]$Flags)
    $s = $script:Story
    $script:Stage = $Stage
    $okCond = switch ($Stage) {
        'build'    { 'a draft PR exists for this story and the local preflight passed' }
        'ci-fix'   { 'the CI failure is fixed, the preflight passed and the fix is pushed' }
        'sync'     { 'main is merged into the branch, every conflict is resolved, the preflight passed and the merge is pushed' }
        'review'   { 'the last review has zero open must findings and the bookkeeping commit is pushed' }
        'finalize' { 'criteria are ticked or marked pending with evidence, the summary is posted and the PR is ready (or a fix was pushed for CI)' }
    }
    for ($attempt = 1; $attempt -le $maxAttempts; $attempt++) {
        $script:Attempt = $attempt
        $script:StageStart = Get-Date
        $script:Phase = $null; $script:PhaseDetail = ''; $script:NowDetail = ''
        $waits = 0; $billed = 0.0
        while ($true) {
            $note = if ($attempt -gt 1 -or $waits -gt 0) { "`nNOTE: a previous run of this stage was interrupted. Resume from the first unfinished step; read .harness/notes/issue-$($s.issue).md; do not redo finished work." } else { '' }
            $prompt = @"
/go-cycle $($s.issue) --stage $Stage $Flags
$note
Run fully autonomously: do not pause for confirmation. Never merge the PR.
When the stage is finished, print the result on the VERY LAST line, nothing after it:
  - exactly "$OK" if $okCond
  - exactly "${BLOCKED}: <one-line reason>" if a stop condition in the skill applies (a human must decide)
  - exactly "${FAIL}: <one-line reason>" if something broke that you cannot get past
"@
            $log = Join-Path $logDir ("issue-{0}-{1}-{2}.log" -f $s.issue, $Stage, (Get-Date -Format 'yyyyMMdd-HHmmss'))
            Write-Log "=== #$($s.issue) $($s.id) stage $Stage (attempt $attempt/$maxAttempts$(if ($s.session) { ', resuming session' })) log: $log ===" 'Cyan'
            $text = Invoke-ClaudeRun -Prompt $prompt -LogPath $log -ResumeId $s.session
            # A --resume that cannot find its session fails fast: retry cold once.
            if ($s.session -and $script:CliExit -ne 0 -and -not $script:SessionId) {
                Write-Log '    session resume failed; starting the stage cold' 'Yellow'
                $s.session = ''
                $text = Invoke-ClaudeRun -Prompt $prompt -LogPath $log
            }
            if ($script:SessionId) { $s.session = $script:SessionId }
            $script:RunCost += $script:LastCost; $billed += $script:LastCost
            if ($script:LimitHit -and $WaitForReset -and $waits -lt $MaxWaits) {
                $waits++
                Write-Analytics -Outcome 'limit' -BilledUsd $script:LastCost; $billed = 0.0
                $script:RunState = 'paused'
                Write-Log "=== LIMIT ($($script:LimitReason)) - waiting for the reset ($waits/$MaxWaits) ===" 'Yellow'
                Wait-UntilReset -Reason $script:LimitReason
                $script:RunState = 'running'
                continue
            }
            break
        }
        $last = (($text -split "`r?`n") | Where-Object { $_.Trim() } | Select-Object -Last 1)
        $last = if ($last) { $last.Trim() -replace '^[`*_]+|[`*_]+$', '' } else { '' }
        $verdict = if ($last -like "$BLOCKED*") { 'blocked' } elseif ($last -like "$FAIL*") { 'failed' } elseif ($script:CliExit -eq 0 -and $last -eq $OK) { 'ok' } else { 'incomplete' }
        Write-Analytics -Outcome $verdict -BilledUsd $billed
        Save-State
        if ($verdict -ne 'incomplete') { return @{ Verdict = $verdict; Reason = ($last -replace "^($BLOCKED|$FAIL):\s*", '') } }
        Write-Log "=== stage $Stage INCOMPLETE ($(if ($script:CliExit) { "claude exited $($script:CliExit)" } else { 'no sentinel' })) ===" 'Yellow'
        if (Test-OverBudget) { return @{ Verdict = 'blocked'; Reason = (Get-BudgetReason) } }
        # No network is not the stage's fault: wait for it and do not count the attempt.
        if ((Test-NetworkError $text) -and (Wait-Network)) { $attempt-- }
    }
    return @{ Verdict = 'failed'; Reason = "stage $Stage incomplete after $maxAttempts attempts" }
}

# --------------------------------------------------------------------------
# Story state (survives a harness restart: .harness/state/<lane>.json)
# --------------------------------------------------------------------------
function Get-StatePath { Join-Path $stateDir "$Lane.json" }

function Save-State {
    if ($script:Story) { $script:Story | ConvertTo-Json -Depth 6 | Set-Content -Path (Get-StatePath) -Encoding utf8 }
}

function New-StoryState {
    param($Pick)
    $saved = $null
    if (Test-Path (Get-StatePath)) { try { $saved = Get-Content (Get-StatePath) -Raw | ConvertFrom-Json -AsHashtable } catch { } }
    if ($saved -and [int]$saved.issue -eq [int]$Pick.number) {
        Write-Log "    resuming saved state: stage $($saved.stage), `$$('{0:N2}' -f $saved.billed_usd) spent" 'DarkGray'
        return $saved
    }
    $meta = Get-NodeJson 'issue.mjs' @('meta', "$($Pick.number)")
    $numeric = @($meta.criteria | Where-Object { $_.numeric }).Count -gt 0
    # A ready (non-draft) PR is finished work that did not merge yet (main was
    # red, or the lane stopped): check CI on it again, then the merge gate.
    $ready = $Pick.pr -and -not $Pick.pr.draft
    if ($ready) { Write-Log "    PR #$($Pick.pr.number) is ready: CI again, then the merge gate" 'DarkGray' }
    return [ordered]@{
        issue = [int]$Pick.number; id = $Pick.id; title = $Pick.title; epic = $Pick.epic; size = $Pick.size; track = $Pick.track
        budget_usd = [double]($SizeFactor[$Pick.size] ?? 5.0) * $BaseUsd; numeric = $numeric
        stage = $(if ($ready) { 'ci' } else { 'build' }); after_ci = $(if ($ready) { 'merge' } else { 'review' })
        session = ''; pr = $(if ($Pick.pr) { [int]$Pick.pr.number } else { 0 })
        claim_ts = (Get-Date).ToString('o'); green_ts = $null
        ci_attempts = 0; ci_failed_platforms = @(); finalize_runs = 0; merge_updates = 0; sync_runs = 0
        billed_usd = 0.0; purecontext_calls = 0; review_rounds = 0; must_total = 0; must_fixed = 0; should_total = 0; rejected_total = 0
        bench = $null
    }
}

function Test-OverBudget { $BudgetOn -and $script:Story.billed_usd -gt $script:Story.budget_usd }
function Get-BudgetReason { 'over budget: ${0:N2} spent of ${1:N0} ({2})' -f $script:Story.billed_usd, $script:Story.budget_usd, $script:Story.size }

function Get-StoryPr {
    $s = $script:Story
    $prs = gh pr list --state open --json 'number,headRefName' 2>$null | ConvertFrom-Json
    $hit = @($prs | Where-Object { $_.headRefName -like "$($s.id)-*" }) | Select-Object -First 1
    return $(if ($hit) { [int]$hit.number } else { 0 })
}

function Get-PrHead { param([int]$Pr) (gh pr view $Pr --json headRefOid -q .headRefOid 2>$null | Out-String).Trim() }

# --------------------------------------------------------------------------
# Finishing a story
# --------------------------------------------------------------------------
function Complete-Story {
    param([string]$Outcome, [string]$Reason = '')
    $s = $script:Story
    $claim = [datetime]$s.claim_ts
    $rec = [ordered]@{
        issue = $s.issue; id = $s.id; epic = $s.epic; size = $s.size; track = $s.track; lane = $Lane; ts = (Get-Date).ToString('o')
        outcome = $Outcome; reason = $Reason; pr = $s.pr; model = $script:ModelArg
        budget_usd = $s.budget_usd; billed_usd = [math]::Round($s.billed_usd, 4)
        ci_attempts = $s.ci_attempts; ci_failed_platforms = @($s.ci_failed_platforms | Select-Object -Unique)
        review_rounds = $s.review_rounds; must_total = $s.must_total; must_fixed = $s.must_fixed; should_total = $s.should_total; rejected_total = $s.rejected_total
        bench = $s.bench; purecontext_calls = $s.purecontext_calls
        claim_ts = $s.claim_ts; green_ts = $s.green_ts
        claim_to_green_sec = $(if ($s.green_ts) { [math]::Round(([datetime]$s.green_ts - $claim).TotalSeconds) } else { $null })
        wall_sec = [math]::Round(((Get-Date) - $claim).TotalSeconds)
    }
    Add-Content -Path $storiesJsonl -Value ($rec | ConvertTo-Json -Depth 6 -Compress)
    if (-not (Test-Path $summaryMd)) { Add-Content -Path $summaryMd -Value "# go-harness run summary`n`n| when | lane | story | outcome | PR | cost / budget | CI attempts (failed OS) | review rounds | must fixed / advisory / rejected | PureContext calls | note |`n|---|---|---|---|---|---|---|---|---|---|---|" }
    $row = '| {0} | {1} | {2} #{3} | {4} | {5} | ${6:N2} / ${7:N0} | {8} ({9}) | {10} | {11} / {12} / {13} | {14} | {15} |' -f (Get-Date -Format 'yyyy-MM-dd HH:mm'), $Lane, $s.id, $s.issue, $Outcome,
        $(if ($s.pr) { "#$($s.pr)" } else { '—' }), $s.billed_usd, $s.budget_usd, $s.ci_attempts, (($rec.ci_failed_platforms) -join ', '), $s.review_rounds,
        $s.must_fixed, $s.should_total, $s.rejected_total, $s.purecontext_calls, ($Reason -replace '\|', '/')
    Add-Content -Path $summaryMd -Value $row
    Remove-Item (Get-StatePath) -Force -ErrorAction SilentlyContinue
    $script:Done[$s.id] = $(if ($Outcome -eq 'merged') { 'done' } elseif ($Outcome -eq 'ready') { 'ready' } else { $Outcome })
}

function Block-Story {
    param([string]$Reason, [string]$Tried = '')
    $s = $script:Story
    Write-Log "=== #$($s.issue) $($s.id) BLOCKED - $Reason ===" 'Red'
    $r = Invoke-Node 'issue.mjs' @('block', "$($s.issue)", '--reason', $Reason, '--tried', $Tried)
    if ($r.Code -ne 0) { Write-Log "    could not label the issue: $($r.Err.Trim())" 'Yellow' }
    Complete-Story -Outcome 'blocked' -Reason $Reason
}

# Wi-Fi, DNS or GitHub gone: the error text of gh, node or the claude CLI.
function Test-NetworkError {
    param([string]$Text)
    return [bool]($Text -match "ENOTFOUND|EAI_AGAIN|ECONNRESET|ECONNREFUSED|ETIMEDOUT|error connecting to|Can't reach the API server|no such host|dial tcp|i/o timeout|TLS handshake|network is unreachable|check your internet")
}

# Waits until GitHub answers again, every network.poll_min minutes. $true when
# it does; $false after network.max_hours (the caller then blocks as before).
function Wait-Network {
    $deadline = (Get-Date).AddHours([double]($Cfg.network.max_hours ?? 48))
    $script:RunState = 'paused'
    try {
        while ((Get-Date) -lt $deadline) {
            Wait-Minutes ([double]($Cfg.network.poll_min ?? 5)) 'no connection to GitHub or the API; waiting for the network'
            gh api rate_limit --jq .rate.remaining 2>$null | Out-Null
            if ($LASTEXITCODE -eq 0) { Write-Log '    network is back' 'Green'; return $true }
        }
        return $false
    }
    finally { $script:RunState = 'running' }
}

function Wait-Minutes {
    param([double]$Minutes, [string]$Why)
    $until = (Get-Date).AddMinutes($Minutes)
    Write-Log "    [wait] $Why - next check at $($until.ToString('HH:mm'))" 'Yellow'
    while ((Get-Date) -lt $until) { Start-Sleep -Seconds 30; Draw-Panel }
}

# Main is red. Re-run its failed jobs once per main commit (a flaky test),
# then check every main_red.poll_min minutes. $true once main is green again;
# $false after main_red.max_hours.
function Wait-MainGreen {
    $poll = [double]($Cfg.main_red.poll_min ?? 10)
    $deadline = (Get-Date).AddHours([double]($Cfg.main_red.max_hours ?? 6))
    $rerun = @{}
    while ((Get-Date) -lt $deadline) {
        $run = @(gh run list --branch $Cfg.base_branch --workflow ci.yml --limit 1 --json 'databaseId,status,conclusion,headSha' 2>$null | ConvertFrom-Json) | Select-Object -First 1
        if ($run -and $run.status -eq 'completed' -and $run.conclusion -eq 'success') {
            Write-Log "    main is green again at $($run.headSha.Substring(0, 7))" 'Green'
            return $true
        }
        if ($run -and $run.status -eq 'completed' -and -not $rerun[$run.headSha]) {
            $rerun[$run.headSha] = $true
            Write-Log "    main is red at $($run.headSha.Substring(0, 7)): re-running its failed jobs once (flaky test?)" 'Yellow'
            gh run rerun $run.databaseId --failed 2>&1 | Out-Null
        }
        Wait-Minutes $poll 'main is red; no merge until it is green'
    }
    return $false
}

# A ready PR a person merges on purpose: the picker does not take it back.
function Set-HumanMerge {
    param([string]$Why)
    $s = $script:Story
    gh pr comment $s.pr --body "go-harness: green and reviewed. Not auto-merged: $Why. A human merges this one." 2>$null | Out-Null
    gh label create 'harness:human-merge' --color 'FBCA04' --description 'Green and reviewed; go-harness leaves the merge to a person' 2>$null | Out-Null
    gh issue edit $s.issue --add-label 'harness:human-merge' 2>$null | Out-Null
}

# The merge gate. merge: human -> leave the ready PR for a person.
# merge: auto -> scripts/harness/merge-check.mjs decides:
#   merge  : green on all OS, reviewed, within budget, up to date with main -> squash-merge
#   update : behind main -> merge main into the branch and run CI again (returns 'again')
#   sync   : conflict with main -> the sync stage resolves it, then CI again (returns 'again')
#   human  : excluded track (platform) or a changed existing fm-api type -> label
#            harness:human-merge, leave it, lane goes on
#   block  : draft/closed, or the merge itself failed -> harness:blocked
#   stop   : main is red -> re-run its failed jobs once, wait for green (main_red),
#            then the gate again; still red after main_red.max_hours -> stop the lane
# Returns 'again' when the story must loop through CI before the gate runs again.
function Invoke-MergeGate {
    $s = $script:Story
    $script:Phase = 'merge'; $script:PhaseDetail = "$($Cfg.merge)"; $script:StageStart = Get-Date
    if ($Cfg.merge -ne 'auto') {
        Write-Log "=== #$($s.issue) $($s.id) READY - PR #$($s.pr) is green and reviewed; merge: human, so a human merges it. ===" 'Green'
        Complete-Story -Outcome 'ready'
        return
    }
    # No budget check here: the money is spent, the PR is green and reviewed.
    # The budget stops new agent work (Invoke-Story), not a merge.
    $r = Invoke-Node 'merge-check.mjs' @('--pr', "$($s.pr)", '--track', "$($s.track)", '--epic', "$($s.epic)")
    $mc = $null; try { $mc = $r.Out | ConvertFrom-Json -Depth 10 } catch { }
    if (-not $mc) { Block-Story -Reason "merge-check failed: $($r.Err.Trim())"; return }
    $why = ($mc.reasons -join '; ')
    switch ($mc.action) {
        'human' {
            Write-Log "=== #$($s.issue) READY - left for a human: $why. The lane goes on. ===" 'Yellow'
            Set-HumanMerge -Why $why
            Complete-Story -Outcome 'ready' -Reason $why
        }
        'stop' {
            Write-Log "=== #$($s.issue) not merged yet: $why ===" 'Yellow'
            if (Wait-MainGreen) { return 'again' }   # stage stays 'merge': the gate runs again
            Write-Log "=== #$($s.issue) READY - main stayed red for $($Cfg.main_red.max_hours ?? 6) h. Stopping the lane; the next run merges this PR. ===" 'Red'
            $script:StopReason = "main stayed red: $why"
            Complete-Story -Outcome 'ready' -Reason $why
        }
        'sync' {
            Write-Log "    $why - the sync stage resolves it, then CI again" 'Yellow'
            $s.stage = 'sync'; $s.after_ci = 'merge'
            return 'again'
        }
        'block' { Block-Story -Reason "auto-merge: $why" }
        'update' {
            $max = [int]($Cfg.auto_merge.max_branch_updates ?? 3)
            $s.merge_updates = [int]$s.merge_updates + 1
            if ($s.merge_updates -gt $max) { Block-Story -Reason "main moved $max times while this PR waited to merge"; return }
            Write-Log "    $why - updating the branch with main ($($s.merge_updates)/$max), then CI again" 'Yellow'
            $out = gh pr update-branch $s.pr 2>&1
            if ($LASTEXITCODE -ne 0) {
                $msg = ($out | Out-String).Trim()
                # GitHub reports mergeable UNKNOWN until it has checked; a conflict
                # then shows up only here. The sync stage resolves it.
                if ($msg -match 'conflict') {
                    Write-Log "    the branch conflicts with main - the sync stage resolves it, then CI again" 'Yellow'
                    $s.stage = 'sync'; $s.after_ci = 'merge'
                    return 'again'
                }
                Block-Story -Reason "could not update the branch with main: $msg"; return
            }
            $s.stage = 'ci'; $s.after_ci = 'merge'
            return 'again'
        }
        'merge' {
            # No --delete-branch: gh also deletes the LOCAL branch, which fails
            # because the lane worktree has it checked out. Delete both by hand.
            $branch = (gh pr view $s.pr --json headRefName -q .headRefName 2>$null | Out-String).Trim()
            $out = gh pr merge $s.pr --squash --match-head-commit $mc.sha 2>&1
            $out | ForEach-Object { Write-Log "    $_" 'DarkGray' }
            if ($LASTEXITCODE -ne 0) { Block-Story -Reason "auto-merge failed: $(($out | Out-String).Trim())"; return }
            if ($branch) {
                git -C $script:Work checkout --detach 2>&1 | Out-Null
                git -C $script:Work branch -D $branch 2>&1 | Out-Null
                gh api -X DELETE "repos/{owner}/{repo}/git/refs/heads/$branch" 2>&1 | Out-Null
            }
            gh issue edit $s.issue --remove-label 'harness:in-progress' 2>$null | Out-Null
            Write-Log "=== #$($s.issue) $($s.id) MERGED (auto). ===" 'Green'
            Complete-Story -Outcome 'merged'
        }
        default { Block-Story -Reason "merge-check returned '$($mc.action)'" }
    }
}

# --------------------------------------------------------------------------
# The story state machine: build -> ci -> review -> ci -> finalize -> merge.
# A red CI loops through ci-fix (budget: ci.retry_budget); finalize that pushed
# a fix loops through CI once more.
# --------------------------------------------------------------------------
function Invoke-Story {
    $s = $script:Story
    $ciFile = ".harness/ci/issue-$($s.issue).json"
    while ($true) {
        Save-State
        # Waiting for CI or merging costs nothing; only stages that run an agent stop.
        if ($s.stage -notin 'merge', 'ci' -and (Test-OverBudget)) { Block-Story -Reason (Get-BudgetReason); return }
        switch ($s.stage) {
            'build' {
                $r = Invoke-Stage 'build' ''
                if ($r.Verdict -ne 'ok') { Block-Story -Reason "build: $($r.Reason)"; return }
                $s.pr = Get-StoryPr
                if (-not $s.pr) { Block-Story -Reason 'build reported OK but no PR exists for the story branch'; return }
                $s.stage = 'ci'; $s.after_ci = 'review'
            }
            'ci' {
                $script:Stage = 'ci'; $script:Phase = 'ci'; $script:StageStart = Get-Date
                $bench = $s.numeric -and $s.after_ci -eq 'finalize'
                $script:PhaseDetail = "waiting for $(@($Cfg.ci.required_checks).Count) OS$(if ($bench) { ' + bench' }) on PR #$($s.pr)"
                Write-Log "=== #$($s.issue) CI: $($script:PhaseDetail) (no tokens spent while waiting) ===" 'Cyan'
                $args2 = @('--pr', "$($s.pr)", '--out', $ciFile) + $(if ($bench) { @('--bench') } else { @() })
                $r = Invoke-Node 'ci-wait.mjs' $args2 -Live
                $v = $null; try { $v = $r.Out | ConvertFrom-Json -Depth 20 } catch { }
                if (-not $v) {
                    if ((Test-NetworkError $r.Err) -and (Wait-Network)) { continue }
                    Block-Story -Reason "ci-wait failed: $($r.Err.Trim())"; return
                }
                if ($v.bench -and $v.bench.metrics) { $s.bench = $v.bench.metrics }
                switch ($v.status) {
                    'green' {
                        $s.green_ts = (Get-Date).ToString('o')
                        Write-Log "    CI green on $(($v.checks | ForEach-Object { $_.os }) -join ', ')$(if ($v.bench) { "; bench: $($v.bench.status)" })" 'Green'
                        $s.stage = $s.after_ci
                    }
                    'red' {
                        # A flaky test or runner hiccup: re-run the failed jobs once per
                        # commit before an agent is paid to "fix" code that is fine.
                        $sha = "$($v.sha)"
                        if ($sha -and $s.rerun_sha -ne $sha) {
                            $s.rerun_sha = $sha
                            $ids = @(gh run list --commit $sha --json 'databaseId,conclusion' 2>$null | ConvertFrom-Json |
                                Where-Object { $_.conclusion -in 'failure', 'timed_out', 'cancelled' } | ForEach-Object { $_.databaseId })
                            if ($ids.Count) {
                                Write-Log "    CI red at $($sha.Substring(0, 7)): re-running the failed jobs once (flaky?)" 'Yellow'
                                # The whole run, not --failed: the bench jobs need the library job's cache.
                                $ids | ForEach-Object { gh run rerun $_ 2>&1 | Out-Null }
                                Start-Sleep -Seconds 20   # let the re-run register before ci-wait looks again
                                continue
                            }
                        }
                        $s.ci_attempts++
                        $failedOs = @($v.failed | ForEach-Object { $_.os }) + $(if ($v.bench -and $v.bench.status -eq 'red') { @('bench') } else { @() })
                        $s.ci_failed_platforms += $failedOs
                        Write-Log "    CI red: $($failedOs -join ', ') (attempt $($s.ci_attempts)/$CiBudget)" 'Red'
                        if ($s.ci_attempts -ge $CiBudget) { Block-Story -Reason "CI failed $($s.ci_attempts) times (last: $($failedOs -join ', '))" -Tried "$($s.ci_attempts - 1) ci-fix stage(s); see $ciFile in the lane worktree"; return }
                        $s.stage = 'ci-fix'
                    }
                    'conflict' {
                        Write-Log "    $($v.note) - the sync stage resolves it" 'Yellow'
                        $s.stage = 'sync'   # after_ci stays: CI, then where this story was going
                    }
                    'infra' {
                        # No code fix helps (billing, spending limit). Keep the story
                        # claimed with its saved state: the next run resumes it here.
                        $script:StopReason = "GitHub did not run CI: $($v.note)"
                        Write-Log "=== #$($s.issue) CI not run by GitHub: $($v.note) ===" 'Red'
                        Save-State
                        return
                    }
                    default { Block-Story -Reason "CI $($v.status): $($v.note)"; return }
                }
            }
            'ci-fix' {
                $r = Invoke-Stage 'ci-fix' "--ci $ciFile --attempt $($s.ci_attempts)/$CiBudget"
                if ($r.Verdict -ne 'ok') { Block-Story -Reason "ci-fix: $($r.Reason)"; return }
                $s.stage = 'ci'
            }
            'sync' {
                $s.sync_runs = [int]$s.sync_runs + 1
                $max = [int]($Cfg.auto_merge.max_syncs ?? 2)
                if ($s.sync_runs -gt $max) { Block-Story -Reason "the branch conflicted with main $max times"; return }
                $r = Invoke-Stage 'sync' "--pr $($s.pr) --attempt $($s.sync_runs)/$max"
                if ($r.Verdict -ne 'ok') { Block-Story -Reason "sync: $($r.Reason)"; return }
                $s.stage = 'ci'
            }
            'review' {
                $r = Invoke-Stage 'review' "--cycles $ReviewCycles"
                if ($r.Verdict -ne 'ok') { Block-Story -Reason "review: $($r.Reason)"; return }
                $s.stage = 'ci'; $s.after_ci = 'finalize'
            }
            'finalize' {
                $s.finalize_runs++
                $before = Get-PrHead $s.pr
                $r = Invoke-Stage 'finalize' "--ci $ciFile"
                if ($r.Verdict -ne 'ok') { Block-Story -Reason "finalize: $($r.Reason)"; return }
                $after = Get-PrHead $s.pr
                if ($after -and $after -ne $before) {
                    if ($s.finalize_runs -ge 2) { Block-Story -Reason 'finalize pushed new commits twice'; return }
                    Write-Log '    finalize pushed a fix; waiting for CI again' 'Yellow'
                    $s.stage = 'ci'; $s.after_ci = 'finalize'
                } else { $s.stage = 'merge' }
            }
            'merge' { if (@(Invoke-MergeGate) -notcontains 'again') { return } }
            default { Block-Story -Reason "unknown stage '$($s.stage)' in saved state"; return }
        }
    }
}

# --------------------------------------------------------------------------
# Worktree for the lane
# --------------------------------------------------------------------------
function Initialize-Worktree {
    if ($NoWorktree) { $script:Work = $PSScriptRoot; return }
    $def = $Cfg.lanes.$Lane
    $wt = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot $def.worktree))
    git -C $PSScriptRoot fetch -q origin 2>$null
    if (-not (Test-Path (Join-Path $wt '.git'))) {
        Write-Log "    creating the $Lane lane worktree at $wt" 'DarkGray'
        git -C $PSScriptRoot worktree add --detach $wt "origin/$($Cfg.base_branch)" 2>&1 | ForEach-Object { Write-Log "    $_" 'DarkGray' }
        if ($LASTEXITCODE -ne 0) { throw "git worktree add $wt failed" }
    }
    $script:Work = $wt
}

# A fresh story starts from a clean origin/main. Leftovers of a stopped story
# are stashed (never discarded).
function Reset-Worktree {
    param([int]$Issue)
    if ($NoWorktree) { return }
    git -C $script:Work fetch -q origin 2>$null
    $dirty = (git -C $script:Work status --porcelain 2>$null | Out-String).Trim()
    if ($dirty) {
        git -C $script:Work stash push -u -q -m "go-harness: leftovers before #$Issue" 2>$null
        Write-Log "    worktree had uncommitted changes: stashed ('git -C $($script:Work) stash list')" 'Yellow'
    }
    git -C $script:Work checkout -q --detach "origin/$($Cfg.base_branch)" 2>$null
}

# --------------------------------------------------------------------------
# KB graduation review (human-gated; unchanged flow from tr-harness, new store)
# --------------------------------------------------------------------------
function Get-KbLessons {
    $script:Work = $PSScriptRoot
    $r = Invoke-Node 'kb.mjs' @('catalog', '--json')
    if ($r.Code -ne 0 -or -not $r.Out.Trim()) { return ,@() }
    return ,@($r.Out | ConvertFrom-Json)
}

function Get-GraduationAdvice {
    param([object[]]$Active, [object[]]$Graduated)
    $candJson = ConvertTo-Json @($Active | ForEach-Object { [ordered]@{ id = $_.id; title = $_.title; category = $_.category; severity = $_.severity; rule = $_.rule; rationale = $_.rationale; occurrences = $_.occurrences; source_issues = @($_.source_issues); file_globs = @($_.file_globs); last_seen = $_.last_seen } }) -Depth 6
    $gradList = if ($Graduated.Count) { ($Graduated | ForEach-Object { "- [$($_.category)] $($_.title): $($_.rule)" }) -join "`n" } else { '(none yet)' }
    $prompt = @"
You are the honest curator of the code-review knowledge base of the Font Manager repository
(Rust core, Tauri 2 + Vue 3 shell). Below are its ACTIVE lessons: each recurred in >= 2 stories
and is already injected into the implementer for matching files. GRADUATING a lesson compiles it
into dev-docs/review-rules.md, which CLAUDE.md imports into EVERY agent's context on EVERY run.
That space is expensive and shared. Graduate only rules that are codebase-wide, keep recurring,
and are high-leverage. Area-specific lessons are better left where they are. Recommending AGAINST
graduation is often right; a minority of "graduate" verdicts is a normal result.

Judge: breadth (any crate/any file vs one area), recurrence pressure (distinct stories, recent
last_seen), leverage (would an always-on line have prevented the cited findings?), overlap with
CLAUDE.md "Rules that do not bend" and the already-graduated rules below.

Already graduated:
$gradList

Candidates (JSON):
$candJson

Do not use any tools. Reply with ONLY a JSON array, one element per candidate, strongest case first:
[{"id":"<id>","verdict":"graduate"|"hold","priority":1,"confidence":"high"|"medium"|"low","why_include":"<1-2 sentences>","why_not":"<1-2 sentences>"}]
"@
    $raw = ''
    try { $raw = ($prompt | claude -p --model $script:ModelArg --output-format json 2>$null | Out-String) } catch { }
    if ([string]::IsNullOrWhiteSpace($raw)) { Write-Host '    ! no recommendation - showing candidates without advice.' -ForegroundColor Yellow; return ,@() }
    $text = $raw; try { $text = [string](($raw | ConvertFrom-Json).result) } catch { }
    $s = $text.IndexOf('['); $e = $text.LastIndexOf(']'); if ($s -ge 0 -and $e -gt $s) { $text = $text.Substring($s, $e - $s + 1) }
    try { $advice = @($text | ConvertFrom-Json) } catch { Write-Host '    ! could not parse the recommendation.' -ForegroundColor Yellow; return ,@() }
    $ids = @($Active | ForEach-Object { $_.id })
    return ,@($advice | Where-Object { $_.id -and ($ids -contains $_.id) })
}

function Split-Wrapped {
    param([string]$Text, [int]$Width, [int]$MaxLines = 2)
    if ([string]::IsNullOrWhiteSpace($Text)) { return ,@() }
    if ($Width -lt 20) { $Width = 20 }
    $lines = @(); $line = ''; $cut = $false
    foreach ($w in ((($Text -replace '\s+', ' ').Trim()) -split ' ')) {
        if ($line -and ($line.Length + 1 + $w.Length) -gt $Width) {
            if ($lines.Count + 1 -ge $MaxLines) { $lines += $line; $cut = $true; break }
            $lines += $line; $line = $w
        } else { $line = if ($line) { "$line $w" } else { $w } }
    }
    if (-not $cut -and $line) { $lines += $line } elseif ($cut) { $lines[-1] = $lines[-1].Substring(0, [Math]::Min($lines[-1].Length, $Width - 1)) + '…' }
    return ,$lines
}

function Format-GradItemLines {
    param($Item, [int]$Index, [bool]$IsCursor, [int]$Width)
    $l = $Item.Lesson; $a = $Item.Advice
    $lineA = @(
        Seg (' ' + ($IsCursor ? '▶ ' : '  ')) $C.boldcyan
        Seg "$($Item.Selected ? '[x]' : '[ ]') " ($Item.Selected ? $C.boldgreen : $C.gray)
        Seg ("{0,2}. " -f ($Index + 1)) $C.gray
        Seg $l.id ($IsCursor ? $C.boldcyan : $C.bold)
        Seg ("   [{0}/{1}] seen {2}x (#{3})" -f $l.category, $l.severity, $l.occurrences, (@($l.source_issues) -join ' #')) $C.gray
    )
    $lineA += if ($a) { Seg ("   agent: {0} ({1})" -f ([string]$a.verdict).ToUpper(), $a.confidence) (($a.verdict -eq 'graduate') ? $C.boldgreen : $C.yellow) } else { Seg '   agent: no advice' $C.gray }
    $lines = @(,(Format-Line $lineA $Width))
    $lines += Format-Line @(Seg ("          {0}" -f $l.title) $C.dim) $Width
    if ($a) {
        foreach ($pair in @(@('+', [string]$a.why_include, $C.green), @('-', [string]$a.why_not, $C.red))) {
            $wr = Split-Wrapped $pair[1] ($Width - 12)
            for ($k = 0; $k -lt $wr.Count; $k++) { $lines += Format-Line @(Seg $(if ($k -eq 0) { "          $($pair[0]) " } else { '            ' }) $pair[2]; Seg $wr[$k] '') $Width }
        }
    }
    return $lines
}

function Show-GraduationPicker {
    param([object[]]$Items)
    $fancy = (-not [Console]::IsInputRedirected) -and (-not $NoPanel) -and ($Panel -or [bool]$env:WT_SESSION)
    if (-not $fancy) {
        for ($i = 0; $i -lt $Items.Count; $i++) { foreach ($line in (Format-GradItemLines $Items[$i] $i $false 100)) { [Console]::WriteLine($line) }; [Console]::WriteLine('') }
        if ([Console]::IsInputRedirected) { Write-Host '    (no console — graduate by hand: node scripts/harness/kb.mjs graduate --id <id>)' -ForegroundColor Yellow; return $null }
        $rec = @($Items | Where-Object { $_.Advice -and $_.Advice.verdict -eq 'graduate' } | ForEach-Object { $_.Lesson.id })
        $ans = ([string](Read-Host ("Graduate which? Enter = recommended ({0}), numbers like '1,3', or 'none'" -f $rec.Count))).Trim().ToLower()
        if ($ans -in 'none', 'n', 'q') { return $null }
        if ($ans -eq '') { return ,$rec }
        return ,@($ans -split '[,\s]+' | Where-Object { $_ -match '^\d+$' -and [int]$_ -ge 1 -and [int]$_ -le $Items.Count } | ForEach-Object { $Items[[int]$_ - 1].Lesson.id })
    }
    $cur = 0; $top = 0
    [Console]::Write("$ESC[?1049h$ESC[?25l")
    try {
        while ($true) {
            $w = [Math]::Max(60, [Console]::WindowWidth) - 1; $h = [Console]::WindowHeight
            $blocks = @(for ($i = 0; $i -lt $Items.Count; $i++) { ,(@(Format-GradItemLines $Items[$i] $i ($i -eq $cur) $w) + '') })
            $head = @((Format-Line @(Seg ' KB graduation review ' $C.boldcyan; Seg ("— {0} active, {1} selected. Graduated rules load into EVERY agent's context." -f $Items.Count, @($Items | Where-Object Selected).Count) $C.gray) $w), (Format-Line @(Seg ('─' * $w) $C.gray) $w))
            $foot = @((Format-Line @(Seg ('─' * $w) $C.gray) $w), (Format-Line @(Seg ' ↑/↓ move   space toggle   r recommended   a all/none   enter graduate   q skip' $C.gray) $w))
            $view = [Math]::Max(4, $h - $head.Count - $foot.Count - 1)
            if ($cur -lt $top) { $top = $cur }
            while ($top -lt $cur) { $used = 0; for ($i = $top; $i -le $cur; $i++) { $used += $blocks[$i].Count }; if ($used -le $view) { break }; $top++ }
            $body = @(); for ($i = $top; $i -lt $Items.Count; $i++) { if ($i -gt $top -and ($body.Count + $blocks[$i].Count) -gt $view) { break }; $body += $blocks[$i] }
            if ($body.Count -gt $view) { $body = $body[0..($view - 1)] }
            [Console]::Write("$ESC[2J$ESC[H" + (($head + $body + $foot) -join "`r`n"))
            $k = [Console]::ReadKey($true)
            switch ($k.Key) {
                { $_ -in 'UpArrow', 'K' } { if ($cur -gt 0) { $cur-- } }
                { $_ -in 'DownArrow', 'J' } { if ($cur -lt $Items.Count - 1) { $cur++ } }
                'Spacebar' { $Items[$cur].Selected = -not $Items[$cur].Selected }
                'R' { foreach ($it in $Items) { $it.Selected = [bool]($it.Advice -and $it.Advice.verdict -eq 'graduate') } }
                'A' { $on = @($Items | Where-Object Selected).Count -lt $Items.Count; foreach ($it in $Items) { $it.Selected = $on } }
                'Enter' { return ,@($Items | Where-Object Selected | ForEach-Object { $_.Lesson.id }) }
                { $_ -in 'Escape', 'Q' } { return $null }
            }
        }
    } finally { [Console]::Write("$ESC[?1049l$ESC[?25h") }
}

function Invoke-GraduationReview {
    $script:PanelMode = $false
    Write-Host "`n=== KB graduation review ===" -ForegroundColor Cyan
    $lessons = Get-KbLessons
    $active = @($lessons | Where-Object { $_.status -eq 'active' })
    if (-not $active.Count) { Write-Host '    no active lessons awaiting graduation.' -ForegroundColor DarkGray; return }
    Write-Host ("    {0} active lesson(s); asking {1} for an honest ranking..." -f $active.Count, $script:ModelArg)
    $advice = Get-GraduationAdvice -Active $active -Graduated @($lessons | Where-Object { $_.status -eq 'graduated' })
    $byId = @{}; foreach ($a in $advice) { $byId[[string]$a.id] = $a }
    $items = @($active | ForEach-Object { [pscustomobject]@{ Lesson = $_; Advice = $byId[[string]$_.id]; Selected = [bool]($byId[[string]$_.id] -and $byId[[string]$_.id].verdict -eq 'graduate') } } |
        Sort-Object @{ Expression = { if ($_.Advice -and $_.Advice.priority) { [int]$_.Advice.priority } else { 999 } } }, @{ Expression = { if ($_.Lesson.severity -eq 'must') { 0 } else { 1 } } }, @{ Expression = { -[int]$_.Lesson.occurrences } })
    $picked = Show-GraduationPicker -Items $items
    if ($null -eq $picked -or -not @($picked).Count) { Write-Host '    nothing graduated - KB unchanged.' -ForegroundColor DarkGray; return }
    $r = Invoke-Node 'kb.mjs' (@('graduate') + @($picked | ForEach-Object { '--id'; $_ }))
    Write-Host $r.Out
    if ($r.Code -ne 0) { Write-Host "    ! graduate failed: $($r.Err)" -ForegroundColor Red; return }
    Write-Host '    done. Commit dev-docs/harness/kb/lessons/ and dev-docs/harness/review-rules.md in a PR (a human decision, so a human commit).' -ForegroundColor Green
}

# --------------------------------------------------------------------------
# Setup checks
# --------------------------------------------------------------------------
function Test-Setup {
    $bad = @()
    foreach ($tool in 'gh', 'node', 'claude', 'git') { if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { $bad += "$tool not on PATH" } }
    gh auth status 2>$null | Out-Null; if ($LASTEXITCODE -ne 0) { $bad += 'gh is not authenticated (gh auth login)' }
    $mcp = (claude mcp list 2>$null | Out-String)
    if ($mcp -notmatch '(?m)^purecontext[^\r\n]*Connected') { Write-Log '    ! PureContext MCP is not connected (claude mcp list). The harness runs, but the loop guard will flag every story.' 'Yellow' }
    foreach ($f in 'Cargo.toml', 'package.json') {
        git -C $script:Work cat-file -e "origin/$($Cfg.base_branch):$f" 2>$null
        if ($LASTEXITCODE -ne 0) { $bad += "origin/$($Cfg.base_branch) has no $f — the workspace does not exist yet (E1.1–E1.3 are human stories)" }
    }
    foreach ($f in 'go-harness.config.json', '.claude/skills/go-cycle/SKILL.md', '.claude/commands/fm-review.md', 'scripts/harness/pick.mjs') {
        git -C $script:Work cat-file -e "origin/$($Cfg.base_branch):$f" 2>$null
        if ($LASTEXITCODE -ne 0 -and -not $NoWorktree) { $bad += "origin/$($Cfg.base_branch) has no $f — commit and push the harness first (the lane worktree checks out main)" }
    }
    return $bad
}

# --------------------------------------------------------------------------
# Main
# --------------------------------------------------------------------------
if ($env:GO_HARNESS_NOEXEC) { return }   # dot-source for tests without running anything

if ($GraduateOnly) { Invoke-GraduationReview; exit 0 }
if ($EpicSync) { $r = Invoke-Node 'epic-sync.mjs' @('--dir', $HarnessDir); Write-Host $r.Out $r.Err; exit $r.Code }
if ($DryRun) {
    $r = Invoke-Node 'pick.mjs' (@('--dry-run') + $(if ($Lane) { @('--lane', $Lane) } else { @() }) + $(if ($Issues) { @('--issues', ($Issues -join ',')) } else { @() }))
    Write-Host $r.Out; if ($r.Err.Trim()) { Write-Host $r.Err -ForegroundColor Red }
    exit $r.Code
}
if (-not $Lane) { Write-Host 'Usage: ./go-harness.ps1 -Lane core|ui [options]   (or -DryRun, -GraduateOnly, -EpicSync)' -ForegroundColor Red; exit 2 }

$ranAny = $false
try {
    Initialize-Panel
    Write-Log "=== go-harness: lane=$Lane | model=$Model | review-cycles=$ReviewCycles | merge=$($Cfg.merge) | budget base `$$BaseUsd | $(if ($script:PanelMode) { 'panel ON' } else { 'panel OFF' }) ===" 'Cyan'
    Initialize-Worktree
    Write-Log "    working in $($script:Work)" 'DarkGray'
    $problems = Test-Setup
    if ($problems) { foreach ($p in $problems) { Write-Log "    ✗ $p" 'Red' }; $script:RunState = 'stopped'; exit 2 }

    $r = Invoke-Node 'epic-sync.mjs' @('--dir', $HarnessDir); if ($r.Out.Trim()) { Write-Log "    $($r.Out.Trim())" 'DarkGray' }

    $count = 0
    $idleSince = $null
    $zeroSeen = $false
    $pickFails = 0
    while ($MaxStories -eq 0 -or $count -lt $MaxStories) {
        $pickArgs = @('--lane', $Lane, '--claim') + $(if ($Issues) { @('--issues', ($Issues -join ',')) } else { @() })
        try { $plan = Get-NodeJson 'pick.mjs' $pickArgs; $pickFails = 0 }
        catch {
            # GitHub hiccups at night: retry a few times before giving up.
            if ((Test-NetworkError $_.Exception.Message) -and (Wait-Network)) { continue }
            $pickFails++
            Write-Log "=== picker failed ($pickFails/3): $($_.Exception.Message) ===" 'Red'
            if ($pickFails -ge 3) { $script:HadFailures = $true; break }
            Wait-Minutes 2 'retrying the picker'
            continue
        }
        if ($plan.action -notin 'start', 'resume') {
            Write-Log "=== lane $Lane $($plan.action.ToUpper()) - nothing to start ===" 'Yellow'
            foreach ($x in $plan.reasons) { Write-Log "    $(if ($x.id) { "#$($x.number) $($x.id): " })$($x.why)" 'DarkGray' }
            # Human-only stories are parked on purpose; show them as a quiet note so
            # the real reason for the wait (printed last, in yellow) stands out.
            $hw = @($plan.waiting_on_human | Where-Object { $_.blocks.Count -gt 0 })
            if ($hw) { Write-Log '    parked for a human (a note, not why this lane waits):' 'DarkGray'; foreach ($h in $hw) { Write-Log "      #$($h.number) $($h.id) -> blocks $($h.blocks -join ', ')" 'DarkGray' } }
            $prs = @($plan.reasons | ForEach-Object { [regex]::Matches([string]$_.why, 'PR #\d+, E[\d.]+') | ForEach-Object Value } | Select-Object -Unique)
            if ($prs) { Write-Log "    -> waiting for the other lane to merge: $($prs -join '; ')" 'Yellow' }
            # The other lane still works: its merge can free a dependency or an
            # overlapping area. Wait and ask again instead of ending the run.
            $maxIdle = [double]($Cfg.idle.max_hours ?? 8)
            if (-not $idleSince) { $idleSince = Get-Date }
            # in_flight 0 once is not enough: the other lane may be between two
            # stories (its last merge just landed, its next claim not made yet).
            # Ask one more time after a poll before ending the run.
            $inFlight = [int]$plan.in_flight
            $recheck = $inFlight -eq 0 -and -not $zeroSeen
            $zeroSeen = $inFlight -eq 0
            if (-not $NoIdleWait -and ($inFlight -gt 0 -or $recheck) -and ((Get-Date) - $idleSince).TotalHours -lt $maxIdle) {
                $script:RunState = 'idle'
                $why = if ($inFlight -gt 0) { "$inFlight story(ies) still in flight" } else { 'nothing in flight; checking once more in case the other lane is between stories' }
                Wait-Minutes ([double]($Cfg.idle.poll_min ?? 10)) "$why; asking the picker again"
                $script:RunState = 'running'
                continue
            }
            break
        }
        $idleSince = $null; $zeroSeen = $false
        if ($plan.action -eq 'resume') { Write-Log "=== resuming #$($plan.story.number) $($plan.story.id) (claimed by the harness) ===" 'Cyan' }
        else { Write-Log "=== picked #$($plan.story.number) $($plan.story.id) $($plan.story.title) [$($plan.story.size), $($plan.story.track), budget `$$($plan.story.budget_usd)] ===" 'Cyan' }

        $script:Story = New-StoryState $plan.story
        if ($script:Story.stage -eq 'build' -and -not $script:Story.session -and $plan.action -eq 'start') { Reset-Worktree -Issue $script:Story.issue }
        Save-State
        $ranAny = $true
        Invoke-Story
        $count++
        $doneId = $script:Story.id
        $outcome = $script:Done[$doneId]
        $script:Story = $null; $script:Phase = $null; $script:Stage = ''
        Draw-Panel

        if ($script:StopReason) {
            $script:RunState = 'stopped'; $script:HadFailures = $true
            Write-Log "=== Stopping the lane: $($script:StopReason) ===" 'Red'
            break
        }
        if ($outcome -in 'blocked', 'failed') {
            $script:HadFailures = $true; $script:ConsecFails++
            if ($StopOnBlock) {
                $script:RunState = 'stopped'
                Write-Log "=== $doneId ended $outcome and -StopOnBlock is set. Stopping the lane. ===" 'Red'
                break
            }
            if ($script:ConsecFails -ge $MaxConsecutiveFails) {
                $script:RunState = 'stopped'
                Write-Log "=== $($script:ConsecFails) stories in a row blocked - looks systemic. Stopping the lane. ===" 'Red'
                break
            }
        } else { $script:ConsecFails = 0 }

        if (-not $NoLoopGuard) {
            $lcArgs = @('--dir', $HarnessDir); if ($BudgetOn) { $lcArgs += '--budget' }
            $lc = Invoke-Node 'loop-check.mjs' $lcArgs
            if ($lc.Code -ge 1) { foreach ($l in ($lc.Out -split "`r?`n")) { if ($l.Trim()) { Write-Log "    $l" $(if ($lc.Code -ge 2) { 'Red' } else { 'Yellow' }) } } }
            if ($lc.Code -ge 2) { $script:RunState = 'stopped'; Write-Log '=== LOOP GUARD tripped - stopping the lane. Triage, then run again (or -NoLoopGuard). ===' 'Red'; break }
        }
    }
    $r = Invoke-Node 'epic-sync.mjs' @('--dir', $HarnessDir); if ($r.Out.Trim()) { Write-Log "    $($r.Out.Trim())" 'DarkGray' }
    Write-Log ("=== lane $Lane finished: {0} | `$$('{0:N2}' -f $script:RunCost) billed | summary: $summaryMd ===" -f (($script:Done.GetEnumerator() | ForEach-Object { "$($_.Key) $($_.Value)" }) -join ', ')) 'Cyan'
}
finally { Close-Panel }

if ($ranAny -and $Graduate -and -not $NoGraduate) { Invoke-GraduationReview }
elseif ($ranAny) { Write-Host '    KB graduation skipped (unattended). Review lessons any time: ./go-harness.ps1 -GraduateOnly' -ForegroundColor DarkGray }
if ($script:HadFailures) { exit 1 }
