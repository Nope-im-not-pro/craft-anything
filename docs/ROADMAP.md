# Roadmap

The shared plan for reaching the product described in the README, for people and agents alike.
`docs/STATUS.md` says what works today; this file says what comes next, who owns it and what an
agent may pick up; [roadmap-history.md](roadmap-history.md) keeps what's finished. Rebuilt on
2026-11-02 from `main` at `2e4afc5`, with v0.3.0 the latest release.

## How to use this file

- Each item has an **id** (`R<milestone>.<n>`), an **owner**, a **status** and a list of
  **pending changes**. Pending changes are small, independently mergeable units. Ids are never
  reused or renumbered: commits, code comments and PRs quote them.
- **Claiming:** before you start, put `— claimed: <name>, branch <branch>` on the pending change
  and push your branch early (or open a draft PR) so others see it. Don't take a change someone
  else has claimed. `— in PR #n` marks one a PR already covers: review that PR rather than starting
  again. The full working rules are in `AGENTS.md` ("Working rules", mandatory). Items owned by
  **maintainers** are steered by the project maintainers: agents may take their *unclaimed pending
  changes*, but design decisions marked **decision** wait for a maintainer.
- **Finishing:** tick the change (`[x]`) where it is, add the commit or PR, update `docs/STATUS.md`
  if what works changed, and add a line to `log/devlog.md`. Maintainers move ticked lines to
  `roadmap-history.md` when a release ships.
- **New work:** add a `- [ ]` line under the item it belongs to; a new item takes the next free id.
- **Status:** `todo` · `in progress` · `blocked (<why>)` · `done`.
- Follow `AGENTS.md` (layering, never crash, everything is a tool, `platform::process` for every
  external program) and the architecture rules below.

## Architecture rules for new work

The project should be easy to fork and bend. New features land behind a seam, not inside a
`match`:

1. **Traits plus a registry, not enums.** Target kinds, checks, builders, asset providers and
   external engines are traits with a registry that holds built-in implementations. Adding one
   means adding a file and one registration line (as tools already work).
2. **Data over code for catalogues.** Stacks, game-engine build recipes, model ids and prices,
   and engine install pins live in typed tables (or TOML files embedded with `include_str!`), not
   in string matches or prompt text.
3. **Headless first; flows are tools.** Everything works without the GUI. Anything a person
   can start from the UI (rebuild, follow-ups, answering questions, engine installs, launching a
   result) is reachable through `Engine::call` and the CLI, so MCP, agents, scripts and other
   frontends get it for free. Orchestration, persistence and process control never live in
   `ui-egui` or `apps/*`.
4. **Versioned files.** Every file we write and read back (`craft.json`, `config.toml`,
   `project.toml`) carries a version, has a JSON schema, and has a migration path.
5. **Structured results.** Decisions such as "did the build pass" read typed fields, never
   English text prefixes.

## Milestones at a glance

| Milestone | Goal | Owner | Where it stands (2026-11-02) |
|---|---|---|---|
| M0 Foundations | Docs match the code, licences checked, one live end-to-end run | maintainers | Done |
| M1 Extensibility | Seams for target kinds, checks, builders, asset providers, engines, plugins | maintainers | Done, except engine pins (R1.5) |
| M2 Product completeness | Every kind of target gets study → build → check → refine with a real check | maintainers | Websites complete; apps and games build and run; API-key builds have never run live (R5.5); agents other than Claude Code are only partly wired (R2.2) |
| M3 Engine depth | More tools, and the known partial ones | open | 314 tools; the open lane |
| M4 Release | Packaging, signing, a managed tier | maintainers | v0.3.0 released, signed, on every platform; a macOS CLI question and the managed tier remain |
| M5 Works for real people | What people hit now that the project is public | maintainers | Comes first |

## Open work

Every unticked `- [ ]` line in this file is open work, under the item it belongs to.
`cargo xtask pending` lists them all, grouped by item (`--count` for the totals). When you find
work that's missing, add it as a `- [ ]` line under the right item in the same PR, rather than
leaving it in a PR description or a chat.

## Order and dependencies

Work in phases. Items in the same phase can run in parallel. A phase says what comes first; what
stops an item from starting is its **Depends on** column, which must be done first. A *soft*
dependency should come first but doesn't block.
Issues are ranked by people affected and how badly, after leaving out what's already fixed.

| Phase | Item | Depends on | Why here |
|---|---|---|---|
| 0 Ship | R5.1 The next release | — | The Windows start-up crash and a build-breaking resume bug are fixed on `main` but not released |
| 0 | R5.2 Keeping contributions moving | — | `main` isn't protected, first-time contributors' CI is held, and a waiting PR fixes a safety bug |
| 1 Builds that work | R5.3 A builder that can't make progress stops | — | Costs people money or quota on every backend; small |
| 1 | R5.4 Local models can build | R5.3 *(soft)* | Largest open gap, with no fix anywhere yet |
| 1 | R2.2 Every builder gets the same tools (that group), and Approvals | **decision** for Approvals | Agents other than Claude Code start but can't do the job, and Clean room's rule doesn't reach them |
| 1 | R5.5 API-key builds fail in words people can use | — | This path has never been run live, and its errors say nothing |
| 1 | R5.6 Tools report only what they saw | — | Confirmed false successes on macOS; golden rule 3 |
| 1 | R5.8 The Claude API and 9router can be picked | — | People with a Claude key can't choose it without an environment variable; 9router isn't known |
| 2 Proof | R5.7 Live checks owed | — | Built but never run for real; each can reopen work above. Lines run when the machine or key they need is at hand, Clean room and the quick start first |
| 3 Depth | The rest of R2.2 | — | Detection waits for reports from 0.3.0 or later; ACP after the adapters work |
| 3 | R2.4 Runtime study, R2.5 Behaviour checks | scripted input (R2.4) before R2.5's sheet check | Makes "ready" mean more for apps and games |
| 3 | R2.7, R2.8 | — | Small gaps left in finished work |
| 3 | R2.9 Everyday use, R2.10 Pinned builds | — | Open to anyone at any time; most lines stand alone |
| 3 Depth | R2.15 Providers as data | R5.8 | Each new provider becomes one row instead of six edits |
| 3 | R2.16 More providers | R2.15 | Small additions once the table exists |
| 3 | R1.5 | — | Small leftovers |
| Any | M3: R3.3, R3.9, R3.11 | — | Open lane |
| Later | R4.1 macOS CLI, R4.2 Managed tier | **decision** | Not before M5 |

---

## M5 Works for real people

### R5.1 The next release
Owner: maintainers · Status: todo
Two reported failures, the Windows start-up crash (#414) and a builder resuming another's
session (#471), are fixed on `main` and still broken for everyone on 0.3.0. This is the
cheapest help we can give anyone, so nothing below should hold it up for long.
- [x] #490 merged (`549ae5e`) with the CLI fix the review asked for: a string-or-object input is
      `any`, so a JSON object given to `--left` stays an object (tested both ways). What it reports
      is 0.2.0's, fixed by #93 (`f6725c2`): on Windows the
      published 0.3.0 CLI with Claude Code 2.1.179 mounted 314 of 315 tools (the 315th is the
      approval tool, which Claude Code doesn't list but calls), and a real Deep build reached Ready
      with `core_action_approve` answering five prompts. The reporter was asked for `--version`.
      macOS is still owed (R5.7)
- [ ] Cut the next release (`docs/releasing.md`). The version is 0.4.0 (#526) and `main` at
      `fb7ac86` is green, but pushing `release` is refused: its ruleset blocks every update and
      lists no one who may bypass it. A maintainer who can push it (or adds a bypass) goes next.
      0.3.0 lacks `0748ef1` (#414: Windows starts DX12 only, the crash behind "it won't open"),
      `1e8f661` (#471: a builder never continues another builder's session) and `3357daf` (#430:
      Claude Code's message on stdin)
- [ ] See that the launcher offers the new version: #74, #104 and #420 installed through it, and
      nothing in this repository says how it picks one
- [ ] After it ships, close what it fixes: #166, #346, #420, #478. #74, #104 and #222 give no
      detail, so they may have another cause: keep them open until their fixes are validated
      against the published release, following `docs/releasing.md`.
      Ask #201 whether `3861132` (#379) ended its early compression (a guess at the link)
- [ ] Close the tickets 0.3.0 already fixed, and the spam: #47, #53, #66, #95, #112, #113, #130,
      #133, #138, #152, #179, #182, #185, #202, #214, #329, #344, #377, #378, #400, #449; spam #194,
      #195. #113, #329 and #377 are 0.2.0's missing CLI, fixed by `1b54cbf`, not what #490 reports.
      Not yet: #64 (its cause is fixed, its retries aren't: R5.3), #142 (R5.3), and #462, which says
      `none`, not #490's failure, and gives no version: ask for it
- [ ] Sometimes starting the app shows nothing: after the release, see from `desktop.log` whether it
      still happens. DX12-only may have ended it; a GOG Galaxy Vulkan overlay layer was seen loading
      into the app
- [x] A program that is found but can't start (an npm script whose `node` is missing) says where it is and
      what to look at, not "isn't installed" (#152, #95); on Windows a program found is started by its
      path, not its name
- [ ] Live-check #152 on Windows 11 with OpenCode from npm, and on a machine with the agent but no
      Node.js
- [x] A start that dies before its window (a native driver crash leaves no dialog or log, #74, #104,
      #33) is remembered: `starting` beside `desktop.log` is written before the window is asked for
      and removed once it's open, and a start that finds it draws with WARP that once
      (`apps/craft-anything/src/start_guard.rs`)
- [x] Live-check the start fallback on Windows: make a start die before its window (kill the
      process at launch), start again, and see `desktop.log` say it drew without the graphics card
      and log `drawing with` an adapter of type `Cpu` (WARP), and that the window opens. Passed on
      2026-11-04 (`main` at `869eb85`, release build, Windows 11, RX 6900 XT): a start killed right
      after `drawing with` the Radeon left `starting`; the next logged "the last start never opened
      its window", drew with "Microsoft Basic Render Driver" (`Cpu`, DX12), logged `window open` and
      removed `starting` — claimed: 00xJS, branch docs/r5.1-start-fallback-live-check
- [ ] The software retry still lets wgpu start every DX12 adapter before `pick_adapter` runs, so a
      driver that crashes while its device is created crashes the retry too. wgpu has no way to
      skip hardware adapters; needs a different route (another renderer, or a helper that starts
      the app with only WARP visible)
- [x] Add to `docs/releasing.md`: before publishing, a Deep build through Claude Code from the
- [x] #794: on some systems DX12 refuses egui's own pipeline (`egui_pipeline`, 0x80070057 from
      `CreatePipelineState`) on AMD, Intel and NVIDIA cards and on WARP too (lightcraft#812,
      pdfcraft#1046, wgpu#10771), so the software retry failed the same way every start, and the
      dialog only said to update the driver. Each failed start now moves on to the next renderer
      (DX12 → WARP → Vulkan → GL, `apps/craft-anything/src/renderer.rs`), keeps the one that worked
      for this app version, and starts again by itself after a wgpu error. Checked with a build
      that failed every DX12 start; the real failure wasn't reproduced (STATUS.md)
      — claimed: 00xJS, branch bugfix/r5.1-renderer-fallback
- [ ] Live-check #794's fallback on a machine where DX12 refuses `egui_pipeline` (the reporter's
      780M, or lightcraft#812's)
- [x] A window that fails after it was shown no longer says "The next start will draw with …": its
      start mark is already gone, so the next start draws as before (#909 review); the dialog says
      it only when the window was never shown (`next_start_note`) — claimed: 00xJS, branch bugfix/r5.1-retry-text-after-shown
- [ ] Add to `docs/releasing.md`: before publishing, a Deep build through Claude Code from the
      finished package, on a Mac and on Windows. Starting the CLI from the package (#91) doesn't
      show whether Claude Code accepts its tools — PR #982; claimed: doubektomas,
      branch docs/r5.1-packaged-deep-build-check. The checklist is documented; live package runs
      remain release validation work.
- [x] Add to `docs/releasing.md`: after a release, close every ticket marked as waiting for it, so
      they don't stay open marked `fixed` — PR #1006; claimed: doubektomas,
      branch docs/r5.1-post-release-tickets. The checklist is documented; closing tickets waits
      for publication of the release containing their fixes.

### R5.2 Keeping contributions moving
Owner: maintainers · Status: in progress
- [x] Fix PR #1030's Windows descendant fixture startup and stop the watched launch before
      startup assertions can fail; cover the failure cleanup path
      — claimed: Codex, branch bugfix/r5.2-watched-launch-tests
- [ ] Run PR #1030's shell-free parent/descendant fixture and startup-timeout cleanup test
      on Windows; the contributor reproduced startup failure and leaked processes on 07671fe
- [x] Clarify PR #1030's test Rustdoc: post-stop heartbeat sampling, child-fixture guard,
      and the direct-run no-write check
      — claimed: Codex, branch bugfix/r5.2-watched-launch-tests
- [x] Address PR #1030 review: take the heartbeat baseline after stopping, guard the descendant
      stand-in against direct ignored-test runs, and document the test helpers
      — claimed: Codex, branch bugfix/r5.2-watched-launch-tests
- [ ] `regression_preview_limits_connections_and_releases_listener` (platform) fails on macOS
      with `Invalid argument` in its socket fixture, also reported before PR #1030's review fixes.
      Investigate the fixture separately from the watched descendant shutdown test
Review comes before new work while the queue is long, smallest and clean PRs first. A PR is merged
only when it's green and a maintainer has fully reviewed it, code as well as wording.
- [ ] Maintainers: protect `main` (PRs required, squash only); the only ruleset covers `release`
- [x] Auto mode's delete check is a safety bug that shows up as Mac test failures. It splits a
      command on spaces, so `rm -rf "C:/x/Game Backup/y"` from a project at `C:/x/Game` goes ahead
      unasked, and it lower-cases paths, so on a Mac or Linux a delete inside a project with
      capitals in its path is refused; `auto_mode_keeps_going_but_never_does_the_dangerous_things`
      fails on a Mac for that. `real_binary_end_to_end_if_present` fails there too, expecting
      `/bin/ls` to be Linux: both fixed (#357)
- [ ] **decision** First-time contributors' CI: GitHub holds their runs for approval, so their PRs
      show no `gate` run (#490 and #413 were among them). Approve each push, or
      change the repository's approval setting
- [ ] #193 changes the language list that #486 (merged) changed too: it needs a rebase. #468 looks
      redundant with #500: ask its author
- [ ] **decision** Wide PRs with no roadmap line: #416 (project goals, 34 files, cites an R2.11 that
      doesn't exist; that id stays free for it) and #503 (two asset providers and spending controls,
      37 files, cites the finished R1.4). Decide whether we want them before more is built on them
- [ ] **decision** Replies don't reach people who file through the ticket relay (each such ticket
      says so), yet the bot asks them for details. About half the open issues came that way. Get
      replies back to them, or stop asking
- [x] `which::tests::usual_windows_dirs_come_after_path` (platform) failed on Windows (10 found, 9
      expected, "nothing twice"): it didn't count Antigravity's second folder, fixed by #770
- [ ] `capture::console_tests::reads_what_the_program_drew_and_stops_at_the_deadline` failed once
      with the whole platform suite running and passes alone (passed 3 whole-suite runs on Windows 11
      on 2026-11-04). Find what it depends on
- [x] `process::launch_tests::a_launcher_that_hands_off_keeps_its_group_until_stopped` failed on a
      busy Windows machine: it killed the run 500 ms after the start, and a `cmd` that hadn't exited
      by then made a cancelled run, not a hand-off. The test now waits for the launcher's mark, and
      a kill that finds the program already exited reports its exit, not a cancel
      — claimed: 00xJS, branch test/r5.2-hand-off-race
- [ ] `website_tools::tests::a_browser_that_exits_at_once_is_reported_at_once` (engine) failed once
      on Windows with the whole engine suite running (66 s under load) and passes alone, three
      times in a row. Find what it times
- [ ] `a_helper_is_judged_by_whether_it_runs` (ai crate) sometimes fails when several crates'
      tests run at once and passes alone: it writes a script and runs it straight away, which a
      busy machine can refuse while another test's child still holds the file open
- [x] `a_remote_query_expands_only_the_remote_working_directory` compares the shell's `$PWD`
      with the fixture's canonical path, so macOS's `/var` → `/private/var` alias does not fail
      the test. Remote expansion and special-character quoting checks are retained — PR #949
      — claimed: Codex, branch test/r5.2-macos-remote-query-path
- [x] `which::tests::usual_windows_dirs_come_after_path` (platform) failed on Windows with 10
      found, 9 expected ("nothing twice"): #748's second Antigravity folder, counted since #770
- [x] `capture::console_tests` (platform) timed out now and then with the whole platform suite
      running. On Windows Server 2022 a program another thread starts while a console opens
      inherits its handles and holds it open until it ends, and closing a console that programs
      are still attached to can hang its host. Program starts and console opens (and Unix terminal
      opens, which have a shorter gap of the same kind) now take one lock (`process::STARTING`),
      and a timeout ends the program's whole tree before closing
      — claimed: SoldierPePe, branch test/r5.2-console-capture-flake
- [ ] A console program that exits by itself and leaves programs attached to its console: closing
      it can still hang the console host on Windows Server 2022 (only a timeout ends the tree)
- [x] `process::launch_tests::a_watched_program_can_be_stopped` leaves its `sleep 30` (Windows:
      `ping -n 30`) running, on `main` too: `stop` ends the shell but not what the shell started
      (#1027). Verified on Windows 11: after the test (5 runs) and after the whole
      `craft-anything-platform` suite plus a 35 s wait, `tasklist` showed no `PING.EXE`. Not
      checked: a panic before `stop`, dropping the handle without `stop`, Windows 10 or Server
      — claimed: yashpatadiya1724-cmyk, branch docs/r5.2-watched-stop-windows-check
- [x] `a_helper_is_judged_by_whether_it_runs` (ai crate) sometimes failed when several crates'
      tests ran at once: a script just written can't start while another thread's child still
      holds it open ("Text file busy"). #787 tries again: on Linux 4 of 72 runs failed before
      it, none after
- [ ] `a_remote_query_expands_only_the_remote_working_directory` (ai crate) fails on a Mac: the
      temp folder is `/var/…`, a link to `/private/var/…`, and the shell's `$PWD` gives the second.
      Compare with the folder's canonical path
- [ ] Revisit the two advisory exceptions in `deny.toml`: `rsa` (no fix; we only verify with public
      keys) when rsa 0.10 lands, and unmaintained `zbus_names`/`zvariant` (Linux-only via keyring
      and accesskit) when those move on. `deny.toml`'s comments pointed at R0.3, which never held
      them; they name this line now — claimed: kuseidos, branch docs/r5.2-deny-points-at-its-own-roadmap-line
- [ ] Check whether the release workflows still run actions that target Node.js 20. None is named
      here any more: every job calls `storytold/craft-common-actions@main`, so a fix belongs there
- [x] Correct STATUS.md's obsolete GUI-only limit: document the shared project feed, engine
      setup, launch and preview tools, with their consent and lifetime limits — PR #1014
      — claimed: Codex, branch docs/r5.2-headless-status
- [x] A spinner on screen no longer makes the window redraw at the display's rate: egui's own asks
      for a frame at once every frame, ours at the animation's pace (`widgets::Spinner`, #579)
- [ ] Measure #579 on Windows 11 (0.3.0 portable): the app's CPU while idle, while a build runs and
      during a website capture, before and after; and why a website capture is slow and sits for a
      long time (no details were given, so nothing was changed for it)
- [x] `remote_discovery_uses_the_remote_program_and_connection_context` fails on a computer with
      OpenCode installed: its made-up path is missing, so `program_of` falls back to `opencode`
      — claimed: justinjohn0306, branch test/remote-discovery-without-opencode
- [x] Idle CPU on an AMD Radeon, measured (2026-11-04, `main` at `869eb85`, release build, Windows 11
      build 26300, RX 6900 XT, driver 32.0.21045.16010, Ryzen 5 2600X; the window open on a fresh
      profile, nothing changing). DX12 on the card: one core at ~100% for as long as the window is
      open; Vulkan: the same; GL: 0%; WARP: ~280% for the first 5 s after start, then 0%. The busy
      thread isn't the app's: on DX12 it starts in `amdxc64.dll`, on Vulkan in `amdvlk64.dll`, and
      it starts with the device, not the main thread. A bare eframe window does the same with every
      present mode and frame latency tried (AutoVsync, Fifo, Mailbox, Immediate; latency 1, 2, 3),
      so nothing in our frame pacing causes or avoids it; GL alone stays idle. Seen in the same way
      with PdfCraft and PhotoCraft (pdfcraft#3255) — claimed: 00xJS, branch docs/r5.2-idle-cpu-amd
- [ ] **decision** On the one Radeon measured (RX 6900 XT, driver 32.0.21045.16010, Windows 11
      build 26300), AMD's idle driver thread costs a core on DX12 and Vulkan; other Radeons
      (integrated ones too), drivers and Windows builds haven't been checked. Say so in the
      troubleshooting docs (with `WGPU_BACKEND=gl` as the way out, which loads AMD's OpenGL
      driver), report it to AMD, or leave it
- [x] Limit the decision line above to what was measured (one RX 6900 XT, one driver, one Windows
      build) and say other Radeons and drivers aren't checked (#912 review)
      — claimed: 00xJS, branch docs/r5.2-radeon-idle-scope

### R5.3 A builder that can't make progress stops
Owner: open · Status: todo
Every round spends the person's money or quota. One that can't do anything shouldn't start.
- [x] Preserve the interrupted request when an agent cannot restore its session: ACP starts a new
      session, or a registered CLI adapter falls back to Plain (#552 review).
      — claimed: Afloat16, branch bugfix/r5.3-interrupted-request-recovery-20261103
- [x] A build stops and says why after two builder turns in a row that changed nothing in `app/`,
      on every backend (#44, #110, #417, #481). The count is saved in the project, so it holds
      across runs: a run ends at 7 checks (`max_rounds`) but check numbers carry on, so #44's check
      28 and #417's 35+ look like several runs (a guess: Try again is the only restart found, and
      Auto mode doesn't start runs) — implementation: `2fc2970948315b64d47359543b458509da42ab3f`
- [x] A builder whose tools didn't start isn't resumed. `turn` (`rebuild/mod.rs`) tries 3 more times
      ("Picking up where it left off", #377) because `worth_retrying` doesn't know this error. Give
      it a typed signal, as `Conversation::session_lost` does, not one more word in that function's
      list. Say what failed and stop (pitfalls: never retry what can't succeed) — commit `f80a16a66834`
- [x] `worth_retrying` decides on English words in a message (AGENTS.md: structured results). Move
      its other cases (sign-in, credit, a missing install) to the same typed signal — commit `f80a16a66834`
- [ ] Find what repeated #64's attempts and stop it: it logged 41, about 38 s apart, on 0.2.0, but
      `turn` allows 3 retries 30, 60 and 90 s apart, there too, so something else starts them
- [x] A rate limit is waited for only on agents with a session: `turn` retries only when
      `agent.session_id()` is set, which API backends and OpenCode's plain adapter never set (read
      in the code, not run). So #148 didn't reach #142, which was OpenCode. Wait and send the turn
      again for them too, then close #142 — commit `f80a16a66834`
- [x] A builder step on an API backend stops after 25 failed build commands in a row (none
      succeeding between), says so and keeps the project (`MAX_FAILED_BUILDS` in `agent.rs`, #399:
      9.4M tokens in 19 minutes of the first stage, never compiled). Try again carries on
- [ ] **decision** Whether a build gets a token or cost limit of its own, beyond rounds and that
      stop (#399): a model that changes code without ever running a build isn't caught by it
- [ ] Check live that the stop fires on a real API-backend build and the message reads well. It
      fired on a local Ollama build (the same built-in loop) after two turns that changed nothing
      in `app/` ("The build stopped after two builder turns changed no files in app/…"); an
      API-key build is still owed
- [x] #736: the build check and the stop message say when `app/` is empty, so a builder that
      claims a build can't hide that nothing was written. v0.3.0 (the reporter's release) had no
      stop after two unchanged turns (#557, merged after it); main stops, and this adds the
      empty-folder wording to the build check (it failed first, before #749's website check) and
      to the stop message — claimed: claude, branch bugfix/r5.3-empty-app-loop

- [x] A build whose checks keep finding no working build stops after 10 in a row, across "Try again"
      presses, until a new request starts the count again (`NOT_WORKING_CHECK_LIMIT`, kept in craft.json as
      `not_working_checks`). "Try again" used to start six more rounds every time with nothing counted across
      runs, so #736's build went on to check 40 and 49 times. A run at the limit stops before a builder turn.
      The unchanged-turn guard already stops a builder that writes nothing; #736's exact Windows run (gemini-cli)
      is not reproduced here. An empty `app/` is named as having no files yet (#771's wording) and the builder is asked to
      write the project and show its build's output, not to fix a build (`NO_PROJECT_PROMPT`). — claimed: fabox-os, branch bugfix/736-stop-empty-build-loop
- [ ] Live-check #736 on Windows with gemini-cli: an empty app/ and a builder that claims `npm run build`
      stops after two turns (or 10 checks) with the "no files yet" note, and Try again doesn't restart it

### R5.4 Local models can build
Owner: open · Status: todo
The built-in agent loop assumes a capable tool-calling model. Reports #24, #44, #110, #403, #417 and
#498 say otherwise (#44's backend is a guess; #116 may be the same), and no local model build has
been run live.
- [x] A live local build through Ollama, small target, to Ready or to a clear stop, and note in
      STATUS.md which models it worked with: Ollama 0.40.7 with `qwen2.5-coder:14b` on Windows
      (RTX 5090, 32k context, no prompt cut) stops clearly at "No project to build found in app/
      yet", twice. The lines below say why
- [x] A builder on a local model is offered 13 tools (about 10,000 characters of descriptions)
      instead of 33-41 (23,000-30,000): the Build tools and the Core tools the brief names, not the
      study set a model explores with instead of writing (#417, `LOCAL_BUILDER_CORE`). Measured as
      text, not on a model; Ollama's default context (4k) was too small for the old list alone
- [x] Run #417's case live (Ollama, `qwen2.5-coder:14b`, "a scientific calculator") before and
      after this: does it call `build_file_write` now? If it still doesn't, the model can't build.
      Yes, with the three fixes below: over about 50 calls it wrote `app/Cargo.toml` (eframe), fixed
      the two errors the checks reported, and check 3 said "Build succeeded". But `src/main.rs` was
      still cargo's "Hello, world!": no calculator was written, and the screenshot check rightly
      failed it at the round limit (#753)
- [x] The system prompt told every model to call `core_brief_write` and `core_plan_update` first and
      `core_handoff_write` last, tools a lean local builder isn't offered, so the model asked only
      for `core_brief_write` and did nothing else for three checks. A lean builder's prompt leaves
      the memory steps out; live, the same model then used the Build tools
- [x] A call written after a sentence ("Let's list the files.\n```json {…}```") isn't run: #554
      reads a reply that is only the JSON. Half the model's calls came this way and the turn ended
      with nothing done. A reply that ends in complete `json` fences, each a call to an offered
      tool, with only words before or between them, now runs them all (`fenced_calls`, ids
      `react_N` as other text calls); words after the last fence, other code, an unclosed fence or
      a tool that wasn't offered keep it text (#753)
- [x] A call followed by a summary isn't run, and the model is told to retry it without trailing
      text: in a second live run of
      #417's case both turns put "### Summary:" after their one `json` fence, nothing ran, and the
      build stopped after two turns with no files. The next message now says that it wasn't run
      — PR #1028, branch bugfix/r5.4-fenced-call-summary
- [x] Keep the retry condition when `Action:` or `Final Answer:` appears between fenced
      calls, without rejecting ordinary words between calls (PR #1031 review)
      — claimed: Codex, branch bugfix/r5.4-fenced-call-followup
- [ ] A local builder spends its rounds on the project file and never writes the app: run 4 had a
      compiling `Cargo.toml` by check 3, its last, with `main.rs` still "Hello, world!". See whether
      more rounds get there, or whether a small model needs the window and the `--screenshot`
      option written for it (a template in the Rust steps)
- [x] The model ran `cargo new app` in `app/` (commands run there), so the project landed in
      `app/app/` and every check said "No project to build found in app/ yet". The check now says
      where a cargo or npm project one folder down is, and the Rust steps say `app/Cargo.toml`,
      not a folder inside `app/` (a brief can't say `cargo init`: not every builder's commands
      start in `app/`). Live, with calls after a sentence run too, the model then made
      `app/Cargo.toml` and the build compiled (#751)
- [ ] The model reads file paths from the project folder and commands from `app/` as one base:
      it ran `cargo init app` and `cd app && cargo build` after writing `app/Cargo.toml`. Say the
      folder in `build_command_run`'s result (or its description) so the two can't be mixed up
- [x] `usage.json` after a local build says about 4,200 input tokens and 6.5 s of work, while
      Ollama logged about six requests of 4,000-5,000 tokens each: find what it leaves out.
      Cached tokens: Ollama 0.40 reports them apart (`cached_tokens`), and the built-in loop's
      `TurnDone` left them out while coding agents count them. It counts them now; a live run on
      Ollama 0.40.7 had recorded 5,323 input tokens of 75,014
      — claimed: SoldierPePe, branch bugfix/r5.4-usage-cached-tokens
- [ ] Told that its `cargo new app` made `app/app/`, the model kept editing `app/app/Cargo.toml`
      and running `cargo build` in `app/`, and checks 2 and 3 still found the project one folder
      down (live, GPU). See whether naming the move gets it out, or whether the run should move it
- [x] The model reads file paths from the project folder and commands from `app/` as one base:
      it ran `cargo init app` and `cd app && cargo build` after writing `app/Cargo.toml`.
      `build_command_run`'s result now says where it ran (`ran_in`), a command that makes a project
      in `app/app/` is told it goes in `app/` itself, and one that starts with `cd app` where
      there's none is told to leave it out. Live (Ollama, `qwen2.5-coder:14b`) the first command
      was still `cargo new app`, before any result could say so. On a GPU (Ollama 0.40.7) the note
      then reached the model; on the CPU a second run wrote the call with a summary after it, so
      nothing ran (the open line on a call followed by a summary)
      — claimed: SoldierPePe, branch bugfix/r5.4-build-command-folder
- [ ] `usage.json` after a local build says about 4,200 input tokens and 6.5 s of work, while
      Ollama logged about six requests of 4,000-5,000 tokens each: find what it leaves out
- [x] An assistant turn with no text sends an empty string, not `content: null`, which Ollama
      refused as `invalid message content type: <nil>` (#403, #536)
- [x] Understand a tool call a model writes as JSON in its text (#417; implementation `dfe9a4db2dcb3e27edbd67311cbb4202584e7af6`). The fallback protocol reads
      only `Action:` and `Action Input:` lines, so anything else ends the turn with nothing done
- [x] Preserve `length` and `content_filter` stop reasons on native tool-call streams; a parsed
      call must not override truncation or refusal and execute tools. Pair other retained
      unexecuted calls with results before the conversation continues — PR #544
- [x] Return a rejected tool argument's JSON Pointer when known and guidance to correct the call
      (#498), keeping schema types strict
      — commit `0c3ae5cca569`
- [ ] Find how each local server is told its context size and set it, or stop with a plain message
      when the prompt won't fit. We assume 24k and send no `num_ctx`, which an OpenAI-style endpoint
      may ignore anyway; LM Studio refused a 12.6k prompt on its 8k default (#73)
- [x] When a server refuses a request for its context window and says the size (Ollama, llama.cpp,
      OpenAI-style), fit the conversation to that window and ask once more, else say how to raise it (#962)
- [x] A per-backend context budget in `[[ai.backends]]` (`LoopConfig::context_tokens` exists), for
      services whose real window differs from 150k (#374) — implementation: `6bd94b84828ec89b805cb8a246f557d0a778c037`
- [x] Find LM Studio on another port (#73): the port in its settings file (when it is there) and
      the ports 1235-1239 are probed like the usual servers; OpenCode's own `baseURL`s were already
      read (#83)
- [ ] Check live that LM Studio on `--port 1235` is offered, and where its settings file is on
      Windows, macOS and Linux (`LM_STUDIO_SETTINGS` is from memory of its folders, not verified)
- [ ] See why Bionic or LM Studio can't be picked (#140)
- [x] Do not select an embedding, ranking or transcription model as a chat backend when a local server has no chat models — claimed: Afloat16, branch bugfix/r5.4-nonchat-model-detection-20261103
- [x] Recognize specialised model file paths and numeric task-name versions during local chat setup
      — claimed: Afloat16, branch bugfix/r5.4-model-task-identifiers-20261103
- [x] The first run's AI step can add a model server on another computer by its address (llama.cpp,
      Ollama or LM Studio on a home server), the same form as Settings → AI
- [ ] A chat sent while a llama.cpp router is still loading the model ended after 31 s with "The
      answer stream ended early." and no next step (seen once live, 27B model); say the server may
      still be loading it and to try again in a minute
      — claimed: yashpatadiya1724-cmyk, branch bugfix/r5.4-stream-loading-remediation

### R5.5 API-key builds fail in words people can use
Owner: open · Status: todo
- [x] `rebuild --backend <an API backend>` still adds the first agent app it finds to the person's
      settings and prints "Using Claude Code (…)": `ensure_agent` (`rebuild_cmd.rs`) runs whenever no
      agent app is set up, though this run won't use one. Run it only when the build needs one
      — claimed: AlexJoaquimPereira, branch `bugfix/r5.5-no-agent-with-api-backend`
- [x] `--backend <a name that is turned off or unknown>` is refused with a specific message: a
      turned-off backend is named and must be turned on, and a name that matches nothing is named
      instead of saying to install an agent or add a key
      — claimed: AlexJoaquimPereira, branch `bugfix/r5.5-disabled-backend-name`; commit a380b3e
- [x] When a project's saved builder was turned off, Start builds with the default AI without
      saying so: the UI picker filters the stale name out (`ui-egui/src/builders.rs`) instead of
      reporting that the chosen AI was replaced. The Make picker says so in a toast, and every
      start (Start, Continue, a note, Try again) names the saved AI and the one building instead
      — claimed: AlexJoaquimPereira, branch `bugfix/r5.5-stale-builder-notice`
- [x] `rebuild --continue-in`, `rebuild_start` on a project and `rebuild_question_ask` name the
      saved builder that is off or gone and the AI building instead: the CLI prints it before the
      run, `rebuild_start` adds `builder_notice`, and an answered question carries it
      — claimed: AlexJoaquimPereira, branch `bugfix/r5.5-headless-builder-notice`
- [ ] The saved-builder replacement notice is English only for now; add it to the catalogs after a
      native speaker checks the wording
- [x] Bound accumulated data across all lines of one server-sent event, so short lines cannot
      grow a streamed API response without limit
      — claimed: Afloat16, branch bugfix/r5.5-bounded-sse-events-20261103
- [x] Show the provider's own reason: #369 bought $10 of credit and got only "OpenRouter answered
      400: Provider returned error". Find the reason in the response body and show it — PR #542
- [x] OpenRouter picks an upstream provider per request, and one without tool calling answered
      "400: Provider returned error" (#369): a request with tools now sets
      `provider.require_parameters`, and a 404 "no provider for these parameters" falls back to the
      text protocol like any server that rejects tools
- [x] A saved fal.ai key reaches an API-key build's asset tools; every call said "No fal.ai key"
      (#923). Outside a project the tools now say that instead. Not checked with a real fal.ai key
      or API build
- [x] An API-key build works alone: it shares the app's session, whose `attach` points at whichever
      project started last, so another build beside it sent its files and asset spending to the
      wrong project (`build.parallel` is 2 by default). Others wait their turn in the queue
- [ ] Let API-key builds run side by side: give each an engine of its own, as a question has. This
      also closes the gap left in working alone: the app and the `rebuild_start` tool each keep a
      queue, so a build the in-app chat starts isn't kept apart from one the app started
- [ ] Check #369 live with Kimi K3 and a key limited to Kimi/Moonshot models (needs the person's
      OpenRouter key): that the build starts, or that the error now names the provider's reason
- [ ] A live API-key build (needs a key on the test machine)
- [x] A Google AI Studio (Gemini API) key is its own choice in Settings → AI and first run (#646):
      the `gemini` backend kind uses Gemini's OpenAI-compatible endpoint and `GEMINI_API_KEY`
- [ ] Check a Gemini build live with a real Google AI Studio key (tool calls through the
      OpenAI-compatible endpoint)
- [ ] Failure logs for the other ways an AI stops (API backends, MCP server start-up), like the
      ones agent CLIs write to `logs/` in the config folder

### R5.6 Tools report only what they saw
Owner: open · Status: todo
Golden rule 3: the result is evidence. Each fix needs a Mac to test on.
- [x] macOS: a denied Accessibility permission is reported as success with an empty tree (#240)
      — claimed: mavericzz, branch bugfix/r5.6-accessibility-denied; PR #887
- [x] macOS: `runtime_ui_read` captures a requested screenshot (#248): one on-screen window
      belonging to the pid, restricted by `window_id` when supplied; PNG/base64 up to 2048 px
      per side. Combined JSON (including escaping and the newline) stays within 16 MiB by
      trimming the tree with explicit truncation and partial coverage, preserving the image.
      Missing Screen Recording permission, a missing window or a capture failure produces
      an explicit screenshot limitation and partial evidence. Denied Accessibility permission with
      the tree enabled (the default) returns a tool error before capture; `accessibility: false`
      skips the tree. Requires macOS 14+ and existing Screen Recording permission; opens no dialog
      — claimed: Codex, branch bugfix/r5.6-macos-screenshot
- [x] macOS: `runtime_ui_read` honours `accessibility: false` (#307): the Swift helper skips
      the tree and its permission check, returns no nodes without truncation, and the evidence
      records the flag and says the tree was skipped. Walkthroughs still require Accessibility
      — claimed: Codex, branch bugfix/r5.6-macos-ui-flags
- [x] macOS: call observation's lldb callback was a literal `\n` in its Python, so the Python was
      invalid and it reported zero calls (#251). Fixed under R2.4: the callback is one valid line
      covering every breakpoint, and a failed lldb run is an error, not "0 calls"; only the live
      lldb run is still owed (R5.7)
      — claimed: mavericzz, branch docs/r5.6-stale-251; PR #1070

### R5.7 Live checks owed
Owner: open · Status: todo
The code for each of these is done; what's left is to run it for real and record the result in
STATUS.md. A failure becomes a new line under its item. Grouped by what the run needs. Start with
the Clean room build and the README quick start: Clean room is the default and has no live build
yet, and the quick start is how every new person begins.

**A signed-in coding agent:**
- [ ] A live Clean room build of a website and of an app, checked end to end
- [ ] A live build in a picked folder each way (edit in place, start from), checked end to end
- [ ] Thresholds under pressure: a clone that's off by a few pixels (a page with fluid type or
      `clamp()` spacing) to see whether 4 / 2 / 8 px is too strict
- [ ] Screenshot check: in a live run the final check once got no picture from an Electron app
      within 90 s while the same check passed in the builder's turn and on a re-run; not
      reproduced. The app waited on animation frames, which a window behind others may not get;
      the Electron steps now say to capture after loading instead
- [ ] A live chat run with a big comparison result, to see Claude Code page it with
      `core_evidence_browse` (and whether 80 KB is the right size)
- [ ] Live check on Windows: rebuild a project while its frozen pin runs, and the build isn't
      blocked

**An API key or a bill:**
- [ ] Run Nano Banana Pro once with a real key (a picture, then a 3D model from it) and check the
      picture's size and the invoice ($0.15)
- [ ] fal bills pictures "by rounding up to the nearest megapixel"; estimates use the exact size.
      Confirm on a real invoice whether a 1024×1024 picture bills as 1 or 2 megapixels, then round
      in the table (needs a key)
- [ ] A live extraction run with Kimi K3.1 Pro (needs the person's OpenRouter key)

**A second computer or another system:**
- [ ] A live run of a project copied between two real computers with Claude Code (#451, #452); done
      on one Mac with a second `CLAUDE_CONFIG_DIR` standing in for the other computer (STATUS.md)
- [ ] Ghidra setup tried for real on Linux and macOS (the Java pins were checked by tests only)
- [x] A Ghidra with no decompiler program for this computer (public downloads leave out macOS) is
      refused before the long analysis, with the command that builds it; checked on a real Mac
- [ ] `engines_install` builds Ghidra's native programs on macOS, so a Mac install works unaided
- [ ] Follow the README quick start on a clean machine (Windows, and Linux or macOS)
- [ ] Build the new DMG on a Mac (release dry run) and look at the window once
- [ ] #490 on macOS: a real Deep build through Claude Code with the published 0.3.0 CLI, with
      2.1.179 and a current version. Windows passed; the Mac CLI was only checked to contain the
      fixed schema
- [ ] macOS CLI zip blocked by Gatekeeper when downloaded in a browser (#94), though Apple has its
      notarization tickets: find why the online lookup fails for people (it passes on our Mac, which
      may be exempt), on a machine that shows it

**A real program, device or file:**
- [ ] Cutter sessions: try them against a real Cutter, including whether `=h&` keeps Cutter
      responsive (only a stand-in server so far)
- [x] Vineflower: run it once on a real JAR (no Java on the machine it was written on)
      — claimed: hasnain607, branch docs/r5.7-vineflower-live-run
- [ ] Verify the 23 unverified adb tools; record results in tool-status.md
- [ ] Android locales: try `only_locales` on a real APK (only the test fixture so far)
- [ ] `app_graphs_compare` on the two real Electron releases again, to count how many of the
      162 "removed" now pair (and pair the `index-<hash>` chunks by structure if many stay)
- [ ] A real IL2CPP `global-metadata.dat` (needs a Unity build or a shipped game); synthetic so far.
      Compare a real APK's components against another tool's reading
- [ ] Stand-in backend: a browser-made HAR and a real front end talking to it (only a hand-written
      HAR so far); GraphQL and tRPC on real bundles (only hand-written minified shapes)
- [ ] Check the Kimi Code, Factory Droid and WorkBuddy registrations in the real apps
- [ ] The Claude API added in Settings with no environment variable: chat with a tool call, cost
      against the console, cached tokens on turn two; then a small build to Ready — waits for
      R5.8's "Add a provider" in Settings
- [ ] 9router 0.5.112 or later: found without a key and with `REQUIRE_API_KEY=true`; a `cc/` model
      and a combo chat; a tool call; a build to Ready; the no-provider error; token saver off in its
      request log — waits for R5.8's 9router and token-saver lines

### R5.8 The Claude API and 9router can be picked
Owner: open · Status: todo
The Claude API is offered only when `ANTHROPIC_API_KEY` is already set (`detect.rs`), and Settings
shows only what Detect found, so a person with a key can't choose it; the first run promises a key
field that isn't there. 9router (a local gateway on port 20128) is unknown, and added by address it
would count as a local model (24k context, lean tools, $0). Decided by the maintainers on
2026-11-07: no environment variable for 9router's key
(keychain or the key file only); one neutral line in docs/9router.md about subscription accounts
behind a gateway, nothing in the UI; Gemini and Kimi stay in Detect's list; no native Gemini API.
- [ ] Provider offers (`ai_providers_list`) and headless set-up tools (`ai_backend_add`,
      `ai_backend_key_set`, `ai_backend_remove`, `ai_backends_list`, `ai_backends_detect`,
      `ai_backend_test`), a key checked before it's saved, CLI `ai providers`, `ai add --provider`,
      `ai key`; `ai` in `registry::AREAS`; `--kind` help names `gemini`
- [ ] "Add a provider" in Settings → AI and the first run, and "Change key" on a saved API backend;
      the first run's promise text matches
- [ ] Claude client: fallback beta header only for api.anthropic.com, model info from `/v1/models`
      (paged, bounded), effort and thinking only where the model has them,
      `model_context_window_exceeded` refits like a context error, a spend-limit 429 is billing,
      Sonnet 5.5 cache-read price
- [ ] `anthropic-version` 2023-06-01 → 2026-10-21 after one live run (decided: move after a live
      run; the body never sends `temperature`, `top_p` or `top_k`) — waits for the live check
- [ ] A gateway on this computer is not a local model: `Locality` (local model / gateway / hosted)
      sets the context budget, the tool list and the cost; Worker's default takes a local model only
- [ ] 9router: found at 127.0.0.1:20128 with or without its key (a 401 asks for one; one that
      answers without a key gets a security hint), its models with tool and combo flags, its errors
      with a dashboard link; any local server that answers 401 is offered with a key field
- [ ] 9router token savers off (`X-9Router-Token-Saver: off`), allowed per backend with
      `token_saver = true` (decided: off by default, with a per-backend option)
- [x] Cap streamed tool arguments (4 MiB) and answer text (32 MiB) from OpenAI-compatible servers
      — claimed: Nope-im-not-pro, branch bugfix/r5.8-openai-stream-caps
- [ ] docs/9router.md, STATUS rows (not run live), file-formats.md says SETTINGS_VERSION 3

---

## M1 Extensibility

### R1.5 External engine trait in L3
Owner: open · Status: in progress
- [x] Find Chrome, Brave, Chromium and Edge installed as Flatpaks on Linux (system and per-user
      exports), and keep a Flatpak or Snap browser's private profile in the home folder its
      sandbox shares, since its /tmp is its own (#747). Unit-tested; not run on a real Flatpak
- [x] Find browsers under their other Linux names, and say why one isn't found: a set path that
      isn't there, or a Flatpak's sandbox (#841). Unit-tested; not run on Linux or in a real Flatpak
- [ ] **decision** Let the Flatpak start browsers on the computer. It can't see system ones today;
      a per-user Flatpak browser in `~/.local/share/flatpak/exports/bin` is found, but its start
      script needs `flatpak`, which the sandbox doesn't have. `flatpak-spawn --host` would need
      `--talk-name=org.freedesktop.Flatpak`, which lets the app run anything outside its sandbox (#841)
- [ ] More pins once their hashes are checked against the publisher: Chromium (no published
      checksums found yet), Rizin for Linux (the tar.xz unpacks to ~800 MB, so it needs streaming
      extraction) and macOS (a .pkg)

### R1.6 Headless first
Owner: maintainers · Status: in progress
- [x] Give Linux-specific Clang guidance in Unreal's guided installer when the bundled compiler
      is missing (the requirements summary is already platform-specific).
      — claimed: AlexJoaquimPereira, branch `bugfix/r1.6-linux-unreal-clang`
- [x] Give macOS the right C++ guidance when Xcode is missing: the status line and
      requirements summary now name Xcode, matching the guided step.
- [x] Find Visual Studio's C++ tools on Windows when Visual Studio is installed outside Program
      Files (through the Visual Studio Installer's `ProgramData` instance state or developer
      environment variables, #669, [PR #726](https://github.com/storytold/craft-anything/pull/726))
      — claimed: riyaz, branch `bugfix/r1.6-windows-vs-cpp-tools`
- [x] Turn off web and capture tools by default in restricted MCP mode (#856, #855).

---

## M2 Product completeness

### R2.2 More coding agents
Owner: open · Status: in progress
- [x] A saved agent program path that no longer exists (Codex desktop moves its CLI to a new
      build folder on every update) says so and names the path, instead of "isn't installed or
      isn't on your PATH" (#721); it stays a missing program, so a builder turn isn't retried
- [x] A relative program path started in another folder is looked up where it was started
      before it's reported as missing (Unix: a script whose interpreter is missing was said to
      have no file there)
- [x] `ai test` names the program it ran and the saved one it replaced, and `doctor` says a saved
      path has no program there instead of blaming PATH (#721)
- [ ] Settings says when a saved agent program is gone and which one runs in its place; today a
      build moves to the newest build folder or the usual program without saying so, and the
      setting keeps the old path (read on a worker, not while drawing) (#721)
- [x] GitHub Copilot through its own CLI login and ACP, without credential import:
      discovery, MCP-only tool restrictions, model selection and fresh read-only questions;
      unit tests and CLI 1.0.105 with an isolated home and loopback model, all five access modes
      — claimed: qu-gg, branch `feature/r2.2-copilot-integration` (recorded during PR preparation)
- [x] Grok Build as a coding agent over ACP (`grok agent stdio`): discovery in `~/.grok/bin`,
      the person's own `grok login` (a missing `auth.json` is unknown, so an API key still
      starts), model and reasoning effort, and our MCP server on the session with `"type": "stdio"`
      only for Grok. Clean room, deep chat, and questions are refused, because Grok can read and
      edit without a permission request. Verified against Grok 1.0.77's ACP handshake and
      `session/set_config_option` (the CLI `--model` flag does not stick after `session/new`). A
      live deep build on 2026-11-06 (Grok 1.0.77) finished, called the Craft Anything server, and
      answered one permission request. Clean room and deep chat were not run live; those runs
      are refused.
      See [Grok Build](grok.md).
      — branch `feature/r2.2-grok-agent`
- [x] Review hardening: cap target handoff at 128 while preserving the active target; propagate
      context-discovery cancellation, normalize CLI-only efforts for Claude requests, and name
      the configured agent in shared picker warnings, with focused regressions and translations
- [x] Local agent chats share a saved investigation and open-target metadata with MCP; background
      refresh makes recorded findings visible in the workbench Evidence pane. `core_finding_edit`
      corrects a finding with checked evidence and an optional revision guard, preserving confidence
      when omitted. Builder refresh failures warn instead of ending the turn; shared ACP errors
      use the current agent and its sign-in guidance. Engine, process-level MCP and headless
      rendering tests cover sharing, mode boundaries and finding ids; regressions cover confidence
      preservation, explicit overrides and agent-specific errors
- [x] Serialize project-backed investigation load, revision check, mutation and all rendered
      writes under one bounded, cancellable interprocess lock. Refuse malformed state rather
      than replacing it; separate-process regressions retain concurrent appends and reject one
      of two updates based on the same revision
- [ ] Refresh externally changed investigation/evidence records incrementally, avoiding repeated
      full reloads under the session lock while retaining mid-turn findings visibility
- [x] Offer OpenCode locally and over SSH with its own visible chat models and supported thinking
      depths (Low, Medium, High, Xhigh, Max) in settings and headless clients. Choices are validated
      before saving; explicit choices are refused at execution until native MCP support is available.
      See [OpenCode](opencode.md).
- [x] Capture OpenCode settings and project together; refuse a busy session instead of treating
      it as no project, and preserve open model pickers and matching results during contention.
- [x] Report cancelled model validation as stopped, while preserving error events for other failures.
- [x] Finish rejected or cancelled model validation with a terminal event so headless chat exits;
      document functions touched by the OpenCode model-selection changes.
- [x] Integrate the five-level `Effort` type from #395 for agent variants, and share a strict
      disk-based selection save between the hub and headless tools, preserving unrelated edits.
- [x] Rebase onto main after #395's squash merge, preserve extra model arguments without a saved
      selection, and render the remote-host placeholder visibly in the setup instructions.
- [x] Hold the UI settings worker's session guard through its disk save so it cannot overwrite
      a newer model selection; reverify the existing terminal-event regression for validation failures.
- [x] Label the settings-save guard so filesystem I/O is identifiable as a running job by the lock tracker.
- [x] Finish a failed CLI worker-thread start with a terminal event after its error so headless chat stops polling.
- [x] Synchronize the UI exit-time settings save with the session's latest AI table and other writers;
      an isolated exit-save regression preserves the latest model/depth and pending app settings.
- [x] Never wait for a running tool's session lock at shutdown: defer non-AI app edits in a versioned
      sidecar when busy, then recover against the latest committed AI settings at startup.
- [x] Discard superseded deferred settings after a successful full exit save; document model-only
      and model-with-variant command forms, and share transport selection between preflight and runs.
- [x] Rebase after #567 and translate the ten model-selection strings and two agent tool titles
      in the Russian catalogue, restoring coverage and catalogue key parity.
- [x] Supply and verify the run's Craft Anything MCP server in native OpenCode before enabling
      explicit model/thinking execution: a config file in `OPENCODE_CONFIG` with `run --standalone`
      (verified live locally). — claimed: advokat11, branch bugfix/r2.2-opencode-native-mcp
- [ ] Hand native OpenCode runs over SSH the Craft Anything MCP server (the config file is local).
- [x] Hand native OpenCode runs over SSH the Craft Anything MCP server (the config file is local).
      One `ssh` run checks `craft-anything-cli` is on the host's PATH and writes the config on the
      host (`~/.cache/craft-anything/`, mode 600, atomic rename); the builder fails fast with the
      fix if the helper is missing. Verified: POSIX-shell regression test (`the_remote_line_runs_on_a_posix_shell`)
      and an SSH round-trip to localhost (probe, byte-exact write, mode, cleanup). Not run with a live
      agent over SSH. — claimed: jelassiaymen94, branch feature/r2.2-opencode-remote-mcp
- [x] Make `the_remote_line_runs_on_a_posix_shell` accept the shell's missing-command status:
      assert failure rather than 127, isolate PATH from installed helpers, and verify that a
      missing helper writes nothing in a fresh home. Fixture utilities use host lookup rather than
      assuming `/bin`; the shell uses the bounded process runner.
      Reproduced the old failure on macOS; the AI suite now passes (446 passed, 2 ignored). PR #1018
      — claimed: Codex, branch test/r2.2-posix-remote-helper
- [x] Find OpenCode's CLI inside its Windows desktop app (`opencode-cli.exe`), for detection and for runs (#1060)
- [ ] Settings → AI can only add an agent it detected: add one by choosing its program, for a CLI
      kept somewhere we don't look (#1060)
- [ ] Verify OpenCode model discovery and an explicit thinking selection on a live SSH host,
      and complete a live generation with the selected model and variant (local catalogue verified).

"Found on this computer" has to mean "can do the job". Today only Claude Code gets everything.
**Every builder gets the same tools:**
- [ ] A table of what each adapter gets (our tool server, approvals, resume, notes mid-turn, a
      message of any length), as typed data, with a test that fails when one is left out.
      `AgentAdapter` already answers some, such as `resumes` and `prompt_on_stdin`. The lines below
      fill it in
- [ ] OpenCode's plain adapter never passes our tool server, so its builder has none of our tools
      and hunts for a setting that doesn't exist (#481). Hand it the server, or build over ACP by
      default once that passes
- [x] Aider has no adapter, so the plain one hands it the prompt as a file name, and it can open a
      browser sign-in nobody agreed to (#72). Give it an adapter, or stop offering it — claimed: Afloat16, branch bugfix/r2.2-aider-recommendations-20261103
- [x] Add a non-interactive Aider adapter before recommending it again; detection currently leaves it out.
      Aider gets the message with `--message`, and flags that stop it asking, updating, editing
      `.gitignore` or opening a page; a chat outside a build also gets `--dry-run`. Before a run it
      refuses when no model or key is set up, or an OpenRouter model has no OpenRouter key: both
      would offer a sign-in and wait for a browser nobody is at (#72).
      — claimed: hasnain607, branch feature/r2.2-aider-adapter
- [ ] Codex (#499): in a chat the tool server has no active target. The reporter attached a patch
      for chat continuity; look at that first
- [x] Codex (#538): list every permitted tool from startup, since Codex doesn't refresh its callable
      list after `enable_toolset` changes it — PR #805
- [x] Codex (#540): `rebuild_start` was refused by `codex exec`'s approval policy and showed as
      `null`; setup and the builder now approve our tools for Codex (we still ask before running
      code) and a refusal explains the fix (`docs/codex.md`)
- [ ] Codex (#540): run `codex exec` on a real install and check `rebuild_start` gets through
- [ ] Codex: resume support (`codex exec resume <id>`), verified on a real install
- [ ] Aider: move the message to `--message-file` once the launcher can write one (see the argv item above), and read project-folder `.aider.conf.yml` and `.env` in `check_setup`, which needs the working directory passed into `check_native_options`.
- [ ] Every adapter but Claude Code's still takes the message in argv (`prompt_on_stdin`), so a long
      one can pass Windows' command-line limit as Claude Code's did (#402); hand each its message on
      stdin or in a file where its CLI reads one
- [ ] A note sent during a build reaches the builder at its next step on other coding agents with
      tool hooks, and on the API-backend loop between tool calls (Claude Code only today)
- [ ] Session copies for other agents that keep sessions (ACP agents' `session/load`) (#451)
- [x] Hermes Agent's native Windows install (`%LOCALAPPDATA%\hermes\bin`) is searched, so an app
      started before the install, or by a launcher, finds it (#116)
- [ ] Live-check #116 on Windows: install Hermes natively, start the app from a launcher, and see
      Settings → AI offer it; #116's "error after every message" has no text yet (asked)
- [ ] See whether Hermes still fails on 0.3.0 (#460: the fix its `fixed` label names was already in
      0.3.0 when it was filed, and it gives no version)

**Approvals:**
- [ ] **decision** One approval behaviour across agents, and whether we want an auto-approve mode
      apart from Clean room and Deep (Auto mode only governs what's asked through us). Today it
      differs by agent: Claude Code asks our approval tool; OpenCode's builder runs with `--auto`;
      Hermes's `-z` turns its approvals off on every run, chats included. Clean room's approval
      rule doesn't reach an agent that never asks us
- [x] Clean room outside builds (#564): a chat or question with Claude Code refuses the agent's own
      file, shell and web tools outright (`--disallowedTools`), not through its defaults; an ACP chat
      refuses them through an approver instead of a consent card (`docs/claude-code.md`)
- [ ] Say on each adapter, as typed data, what it can be held to (asks us, its own sandbox,
      nothing), in a build, a chat and a question, and have runs act on that, once the decision above
      is made. Today Codex, Gemini CLI, OpenCode, Antigravity, Hermes and Maki are not held to
      Clean room in a chat (their own sandbox, if any, is not established)
- [ ] Check live that a Clean room chat with Claude Code can't read a file outside its folder, and
      that an ACP chat in Clean room still reaches our tools (its permission requests for them are
      allowed, only its own read/write/command/fetch are refused)
- [ ] Check live what Hermes over ACP asks about: whether its file reads reach our approval policy
- [ ] Maki's plain `maki -p` builder doesn't ask us before commands, and in Clean room its reads
      aren't checked (as with every agent that never asks us): say so in the approval decision
      above, or run Maki over ACP by default once that passes. Outside a build, tools the person
      allowed in Maki's own permissions (an MCP server's, or all of them with `always_yolo`) still
      run; the adapter can't see that config
- [ ] Maki over ACP writes inside its working folder without a permission request (its built-in
      rule), and that folder is a build's project folder, so writes to our own records (inbox,
      manifest) skip the approval policy; `maki acp` takes no tool flags to stop it (0.7.0). Start
      it in `app/` (paths in the prompt relative to that), or ask Maki for a way to make those
      writes ask; part of the approval decision above

**ACP:**
- [ ] Gemini CLI, opencode, goose via ACP or native adapters; each verified live, then turn ACP on
      by default for the ones that pass
- [x] Over ACP, suppress `session/update` notifications until `session/load` answers, keeping
      replayed messages and tool activity out of the current turn
      — commit `38ffc39be084`
- [ ] Over ACP, turns report 0 tokens: the protocol sends only `usage_update` (context used and
      size). Read real counts where an agent keeps them (Maki: `token_usage` in its session file
      under `~/.local/state/maki/sessions/`) or say the count is unknown instead of 0

**Finding them:**
- [ ] #105, #135, #259, #385 and #423 predate 0.3.0, which looks in more places, and came through
      the ticket relay, so nobody can ask them for `doctor` output. Close them pointing at 0.3.0,
      and add search paths only for a new report that has that output
- [x] #423: an agent that is set up but switched off is never offered again, so "No coding agent was
      found" stays on screen with Claude installed: the setup card and "Look again" leave out every saved
      AI by name, on or off. Offer a switched-off agent as found and turn it on, keeping its options.
      Cause is not confirmed on the reporter's machine (needs their config.toml); the fix covers the
      switched-off case that the code leaves out (`is_on`, `leave_out_set_up`, `builder_backend`). — claimed: fabox-os,
      branch bugfix/423-disabled-agent-not-offered
- [x] Say plainly when what's found is the Claude desktop app rather than the `claude` program
      (#259, a guess at its cause)
- [ ] macOS: check that `Claude.app` is where the desktop app is found (#259's hint is untried there)
- [x] #259's hint, the gaps left in review of #781: it's hidden when only `~/.ssh/config` suggestions are
      found, a Store install without its `claude-desktop` alias isn't seen, and a desktop app's
      `claude.exe` on PATH is taken for Claude Code (#824) — claimed: FuryNocturn, branch
      bugfix/r2.2-claude-desktop-hint-gaps
- [x] macOS: `/Applications/Antigravity.app` ended up as the agent's command (#467). Found: the "Where is
      the program?" picker (`pick_file`) returns the bundle, and `set_command` saved any path. Refuse a
      folder or app bundle when it's saved (`set_command`), and don't run one that was saved before
      (`program_of` uses the usual name). Tests: `an_app_bundle_is_refused_as_an_agents_program`, `program_of`
      cases. — claimed: fabox-os, branch bugfix/467-agent-program-app-bundle
- [x] Windows: look in `%LOCALAPPDATA%\agy\bin` too (#62) — claimed: Afloat16, branch bugfix/r2.2-antigravity-search-20261103
- [x] Windows: `usual_windows_dirs_come_after_path` counts Antigravity's second folder (#748
      added it; the test runs only on Windows, which CI doesn't)
- [ ] CI runs the tests on Ubuntu only, so `cfg(windows)` tests (search paths, consoles, Job
      Objects) are checked only on contributors' machines: run them on a Windows runner too
- [ ] "Claude Code on <ssh host>" is offered for any SSH host, whether or not it has Claude Code
      (#62)
- [ ] Gemini CLI's sign-in reportedly refuses individuals now (#400): check, and stop recommending
      it in the builder setup dialog if so
- [x] DeepSeek Harness: an adapter runs `dsh --profile headless "<task>"` and hands it our tool
      server as a generated `--patch` overlay for that run (its own project, inbox and mode), since
      the harness reads tool servers only from profile layers; a question gets none and the person's
      own profile is left untouched. Verified with adapter and overlay tests and a live follow-up
      website build whose journal shows `rebuild_check`, `core_unknown_add` and `core_handoff_write`
      going through our server — claimed: 0231071, branch `feature/dsh-agent-adapter`
- [x] DeepSeek Harness: send tasks on stdin so multiline and long messages can start through the
      Windows desktop launcher (see the argv note above)
      — claimed: gtxy27, branch `bugfix/r2.2-dsh-stdin`
- [ ] DeepSeek Harness: no resume (its headless profile keeps no session), and a build on the harness's
      desktop profile is untried
- [ ] `setup` writes the plain `mcp` command, so it drops an `--all-tools` an entry already has, and
      `doctor` then calls that entry "a different copy or shape of the command" and suggests the very
      command that breaks it. The installed skill now warns against running `setup` on such an entry;
      `doctor` and `setup` should keep the person's extra arguments instead
      — claimed: 18923837179wjt-dot, branch fix/setup-skill-warns-before-setup-drops-all-tools

### R2.4 Runtime study for apps and games (with consent)
Owner: maintainers · Status: in progress
- [ ] UI tree on Linux (AT-SPI)
- [x] `native_window_capture` on macOS: no Screen Recording is refused before the program starts
      (it was "didn't show a window", which said to wait longer), and the name fallback matches
      whole words and never the system, so `win` no longer pictures the menu bar
- [x] `native_window_capture` keeps looking for up to 10 s past `wait_secs` when no window is there
      yet, instead of deciding from one look; tried with a program that opens its window late
- [ ] Close a pictured program politely before killing it: a helper it started through launchd
      outlives the process group, and can stay running with the program's data files open
- [x] `apple_calls_observe`: the breakpoint callback is one valid line (it had a literal backslash
      and `n` between statements, so lldb stopped before launching, #251), covers every breakpoint
      instead of only the first, a failed lldb run is an error and not "0 calls", and names or
      arguments with line breaks are refused (they'd add commands to lldb's command file). The
      callback body was compiled with Python; lldb itself was not run
- [ ] Run `apple_calls_observe` on a Mac against a small program with `lldb` (the #251 probe):
      the hit for `alpha` is recorded, and a program lldb can't debug says so
- [ ] Window capture tried on macOS and Linux (only Windows so far); games behind a launcher
- [ ] Short scripted input sessions (enigo) to record screens of the original

### R2.5 Behaviour checks for non-web builds
Owner: open · Status: in progress
- [ ] Behaviour checks that use the sheet: run the build with scripted input and measure values
      (speed, jump height, fire rate) against it (needs R2.4's scripted input)
- [ ] Hold the self-test to what the check can see too: picture the game after its scripted
      input and require it to differ from the `--screenshot` picture (a report alone is the
      build's own word)

### R2.7 The request decides how to build
Owner: open · Status: in progress
- [ ] Make: a game whose request names a technology the stack table doesn't know (Phaser, Bevy…)
      still needs the default engine installed to start, though the builder is told to use what
      was named; ask the person, or let such a game start without an engine
- [x] The website check's missing-site note names every place `site_dir` accepts (dist/, build/,
      out/, or a plain index.html with no package.json) and the app's real place, "app/" or "the
      project folder" (#494)
- [x] A project edited in place (Work on my project, or `rebuild --existing edit`) keeps what its
      files are built with, unless the request names another stack: an engine's project file, then
      package.json and Cargo.toml dependencies, then `found_by` in `stacks.toml`, and a project
      whose dependencies name no stack there is left to the builder. A request that read as a game
      used to ask for Unity even in a Godot project, and an app got the kind's default
      — claimed: xerenon, branch bugfix/r2.7-stack-from-files; PR #755
- [x] A person's own project (edited in place, nothing to recreate) is judged by what it does for
      itself: its own build, then its own tests, through `[[toolchain]]` in `engine/src/toolchains.toml` (Rust,
      Node.js, Python, Go, .NET, Maven, Gradle, Make). Nothing is added to it for our sake: no
      `--screenshot`, `--self-test` or mechanics sheet. A site stack with nothing to recreate is a
      website, whatever the request says
      — claimed: xerenon, branch feature/r2.7-own-project-checks; PR #760
- [ ] Run the Go, .NET, Maven and Gradle rows of `[[toolchain]]` on real projects, and every row
      on Windows (`windows_program`): Rust, Node.js, Python and Make were run for real, on a Mac
      only. Also a person's own Unity project: it's opened in batch mode, which should exit with
      an error when its scripts don't compile (not run: no Unity on that machine)
- [ ] A person's own tests that already failed before their request fail the check too, so the
      builder is sent to fix them. If that proves a nuisance, run them once before the first turn
      and hold it only to what it broke
- [x] New projects build through the shared `[[toolchain]]` catalogue. Python root entrypoints,
      Go's root `main.go`, and one root `.csproj` also launch for screenshots, self-tests and Play;
      existing packaged programs, Rust, Electron and game-engine paths keep priority — PR #950
      — claimed: Codex, branch bugfix/r2.7-new-project-toolchains
- [x] Runtime window capture follows runner descendants (including `go run`) and reads controls
      from the same process that supplied the picture, without falling back to unrelated names — PR #950
- [x] Shared toolchain commands make project-local executables absolute before changing the
      working directory, preserving virtualenv paths for relative project placements — PR #950
- [x] Launch a Go project with one `cmd/<name>/main.go` as a package when the root has no
      `main.go` — claimed: Afloat16, branch feature/r2.7-go-command-layouts
- [ ] Extend launch recipes to Python package scripts and Java build outputs; build checks already
      use their toolchain commands
- [ ] Audit relative project placements in build, option-run and Play paths too: runtime window
      capture now normalizes its request paths, but earlier runner paths can still be relative
- [x] "player" alone doesn't make a request a game: a video or music player is an app (#953)
      — claimed: justinjohn0306, branch bugfix/953-player-not-a-game
- [x] System-tool and media-player phrases keep the app target: "a low level disk tool",
      "an MPEG 2 player" and "a single player app for movies". Independent game cues still apply
      — claimed: Afloat16, branch bugfix/r2.7-media-and-system-apps

### R2.8 Clean room means clean room
Owner: maintainers · Status: in progress
- [ ] A remote (SSH) builder needs a craft-anything-cli that knows `--deep` and `--clean-room`:
      check its version first and say so when it's older

### R2.9 Everyday use: typing, projects, settings
Owner: maintainers · Status: in progress
Gaps found while using the app for real projects.
- [x] Each project chooses, per kind of action, what the builder's approvals get (allow, ask or
      refuse): reading the web, commands, files outside the project, pushing and publishing,
      deleting outside the project, system changes, other tools. Unset kinds follow Auto mode as
      today. Found when Auto mode refused every `git push` without a card, so the person's OK in
      the prompt box never reached the check, and refused `gh repo view` as publishing too.
      Tried live by a maintainer on Windows 10 (a release build of `main` at `b700e1d`,
      2026-11-06): a builder's push was approved and went through
      — claimed: tanish2k09, branch feature/r2.9-project-approvals
- [ ] Agents that don't ask our approval tool (OpenCode `--auto`, Hermes `-z`, Maki's `-p`) aren't
      held to a project's approval choices either; part of R2.2's approval decision
- [ ] While "commands" are allowed (Auto mode's default), a builder can run any program. Commands
      that name `craft.json`, the inbox or the approvals tool count as system changes, and our CLI
      started inside an agent calls tools as an agent, but a script that builds the path, or
      unsets `CRAFT_ANYTHING_AGENT`, isn't caught. Keep the person's choices where a builder's
      shell can't write them, or say plainly that "commands: allow" trusts the builder
- [ ] A typed note ("yes, push it") still reaches the builder only as text: say in the card's
      place, when the builder is refused, how the person can allow that kind
- [x] The project page and a slash command change those choices (like `/btw`): the Approvals
      menu under the prompt box and `/approvals <kind> <allow|ask|refuse|default>`
      — claimed: tanish2k09, branch feature/r2.9-approvals-ui
- [ ] The new approvals strings in every catalog were written with an AI; not reviewed by native
      speakers yet — pt-BR claimed: marlonferreira1800-hue, branch docs/r2.9-approvals-pt-br-review
- [x] Keep New project's content column fixed while the vertical scrollbar animates in or out,
      hiding it when there is nothing to scroll, so switching tabs never shifts the page horizontally
      — claimed: Codex, branch bugfix/r2.9-home-scrollbar-gutter
- [x] Agent paragraphs with glossary terms and long file links wrap together without pushing
      their card or later messages under the sidebar (#523); Project and Ask bounds checked at two sizes
      — claimed: carl-dourado, branch hotfix/r2.9-523-message-width
- [x] Create atomic-write temporary files exclusively with short names distinct from the destination and private Unix permissions from creation.
      — claimed: Afloat16, branch bugfix/r2.9-atomic-temp-creation-20261103b
- [x] A prompt brought back with Up keeps its pictures (their paths are absolute, so they don't
      come back after the project is moved)
- [ ] Change a project's builder from its page (a tool, so the CLI and MCP can too) — in PR #245
- [x] `chat --backend <a name that is turned off>` and `compare` refuse it before model or
      agent setup: `AiHub::start` checks `enabled` and says how to turn it on or choose another
      — PR #1008; claimed: Codex, branch bugfix/r2.9-disabled-chat-backend
      (the API regression uses invalid tunnel settings to detect setup before the guard)
- [x] Ctrl+V (Shift+Insert, Cmd+V) in a prompt box that takes pictures pastes a copied image (saved
      as a PNG) or copied picture files; attachments show as square picture tiles, the whole
      picture on hover, decoded on a worker
- [x] A picture over 20 MB dropped on a prompt box or picked in its dialog is refused when it's
      added, as a paste does (`workspace::attachable_picture`), before anything is copied
- [ ] When the builder decides a question, show the question and its choice as an answered card
      in the feed (the run sees only the tool call today; the builder's words say what it chose)
- [ ] A question's own cost: a forked Claude Code session reports the whole session's running
      total, so a forked answer shows none; count questions in the project's usage
- [ ] "Make this change" on an answer that suggests one (sends it as a normal message)
- [x] "Just ask" is told it has fewer tools than the builder and that the build may be running
      beside it, so it no longer reports the build's tools as disconnected or its work as cut off
- [x] "Change it" knows what was just talked about in "Just ask": the next message tells the builder
      the latest few questions and answers, once, and a Clean room build only those asked in Clean
      room (#857). Not checked in a live build
- [x] A note sent while a build is running tells it what was asked in "Just ask" (#857): the
      note's hand-over file carries the answers, and the after-turn path puts them before the
      notes; only answers the run's mode may show, marked told when read or sent
      — claimed: AlexJoaquimPereira, branch bugfix/r2.9-note-just-ask; commit 78d3963
- [ ] Record which mode a builder's session ran in: `agent_session` is kept across modes, so a Clean
      room build or question can resume a session from a Deep build and carry what it read, and such
      a question's answer is recorded as asked in Clean room. Don't resume one for Clean room
- [x] #504: the builder's plan card can be folded away with its X, or with `project_plan_fold`
      (between builds: a fold during a build can be overwritten by its next save). The steps stay
      in the conversation (`FEED_VERSION` 4, `FeedItem::PlanDismissed`), and a later run's
      checklist shows again — claimed: claude, branch bugfix/r2.9-plan-card-dismiss
- [x] A run that ends with steps still open marks them as stopped on its card, instead of
      showing "Doing" until someone folds it (#504: Claude's runs left the plan unticked)
      — claimed: rust-craft, branch feature/R2.9-stop-open-steps; PR #876
- [x] #666: a long line in a feed's tool-work row (a Deep-mode decompile, a status note) ran off
      the window and pushed the feed left; the header title can't wrap in egui, so it's the note's
      first line now, cut at 90 characters, and the full note wraps in the body — claimed: claude,
      branch bugfix/r2.9-feed-text-overflow
- [ ] **decision** A program started from the CLI (`project_play`) keeps the shell's output pipe
      open, so the command doesn't return until it closes (#207). Every fix found needs `unsafe` or
      an unstable API: an exception to rule 9, or a limit we document. Meanwhile a reproducing test
      marked `#[ignore]` and a line in the CLI docs are welcome
- [x] macOS: the window drew Windows-style controls at the top right, since the app turned off the
      system's title bar (`with_decorations(false)`); it now keeps the system's buttons, frame and
      resize edges under a see-through title bar, which is 32 pt there so the buttons sit centred
      in it (#20)
- [ ] macOS: the app's icon is bigger than the system's (#20)
- [ ] Show a command's output while it runs: Claude Code reports a tool's result only when it
      ends, so a long `cargo build` is one quiet line until then. Watching the agent's child
      processes from outside could say what's running and for how long
- [x] Release the keep-awake helper before a rebuild reports `Finished`, waiting up to five seconds
      for its supervised cleanup and logging a timeout. Real subprocess stand-ins checked on Linux (#541)
      — claimed: Afloat16, branch bugfix/r2.9-keep-awake-cleanup-20261103
- [x] Recheck keep-awake cleanup with a real Windows CLI `rebuild`: the helper once outlived it by
      more than 9 minutes and held the caller's output pipe; not reproduced in 8 more runs. Found:
      closing the desktop app during a build ends it without dropping the guard, and the helper
      (an endless PowerShell loop, started with a week's time limit) carried on. It now ends by
      itself once the process it serves is gone: the Windows loop checks that process every 30 s,
      and macOS uses `caffeinate -w`. Live on Windows: a CLI build killed outright, its helper
      ended 30 s later. Not run on a Mac
- [x] Zoom (Cmd/Ctrl + / − / 0) between 50% and 200%: egui's own limits are 20% and 500%, where
      text is unreadable or the window has room for almost nothing
      — claimed: sanketdofe, branch feature/r2.9-zoom-bounds
- [ ] Find why a site capture lost its browser ("the WebSocket is closed", 419 ms into
      `web_site_capture` for a second page, seen once; the same capture passed 41 s earlier)
- [x] Delete a private browser's temporary profile when it closes or fails to start (#810) — PR #822
- [ ] Stop private browsers that outlive the app after an abrupt exit, and investigate the Brave
      Helper crash (#810); both still need macOS reproduction
- [ ] Claude Code's Ultracode switch (a setting, not an effort level; the effort environment
      variable we use can't turn it on)
- [x] A model the list doesn't offer (`opusplan`, `[1m]` variants, one another harness serves) can
      be typed in beside the model list in Settings → AI, as `ai model` already allowed (#470)
- [x] The preview server answers only requests addressed to this computer (a Host and Origin
      check against DNS rebinding), streams large files instead of reading them into memory,
      sends no body for HEAD on a missing file, and caps open connections and the time a
      request may take to arrive (#597, #598, #600, #601, by Afloat16)
- [x] The command palette stays where the wheel scrolls it; it follows the selection only when the
      arrow keys or typing move it (#584)
- [x] Linux: draw on the GPU that drives the screen. egui-wgpu's default picks the discrete GPU, so
      on a laptop whose integrated GPU drives the panel the window took 20 s to open and the whole
      desktop lagged (#571). Pick by the connected displays in `/sys/class/drm`: the built-in
      panel's GPU, else the only GPU with displays, else the discrete one; `WGPU_POWER_PREF` and
      `WGPU_ADAPTER_NAME` still decide when set. Checked live on an AMD + NVIDIA laptop only (#578)
      — claimed: Nghiathan13, branch bugfix/r2.9-linux-gpu-choice
- [ ] Linux GPU choice on a desktop with monitors on two GPUs, and on an Intel + NVIDIA laptop:
      covered by unit tests on fake `/sys/class/drm` folders, not run on such a machine yet (#571)
- [x] "Work on my project" on New project, beside "Make my own version" and "Just look inside":
      pick the folder of a project you already have, say what to change, and its files are edited
      in place, with nothing asked; the project takes the folder's name, an empty folder is
      refused and a folder that's already a project opens. Until now that was only reachable
      through "Choose a folder…" under Make's Start, on a screen that asks what to recreate
      — claimed: xerenon, branch feature/r2.9-work-on-my-project; PR #686
- [x] Leaving "Work on my project" forgets a folder it would edit in place and drops a folder
      dialog that ends later, so a version made on Make can't edit that project, and an address
      in its request isn't read as an original to recreate (review of #686) — claimed: xerenon, branch bugfix/r2.9-own-project-review; PR #750
- [x] `build_files_list` lists an edited-in-place project from the project folder itself, not a
      missing or nested `app/`, and leaves Craft Anything's own records out
      — claimed: AlexJoaquimPereira, branch `bugfix/r2.9-build-files-in-place`; commit 1ffeb40
- [x] Choose Claude Code's folder (`CLAUDE_CONFIG_DIR`) per AI, so one computer can build with
      two Claude accounts; an app opened from the Dock never sees the shell's variable. Settings →
      AI (a typed folder, and Add another) and `ai folder`; live-checked from the CLI with an empty
      folder (not signed in, and the fix names the folder) overriding the shell's own variable,
      and by a build in the app with a typed `~/` folder. A sign-in that expired says so instead
      of "isn't signed in" (the folder's `.claude.json` still names the account)
      — claimed: Claude (Opus 5.5), branch feature/claude-config-dir
- [ ] A live build with two Claude Code AIs signed in to two accounts, and one over SSH with a
      `~/` folder (the remote line is unit-tested, not run on another computer)

- [x] The Advanced console's status stays clear of its tabs when the console is narrow
      — claimed: justinjohn0306, branch bugfix/r2.9-console-status-overlap
- [ ] Keep the builder-availability smoke regression independent of live agent discovery.
      — claimed: Afloat16, branch test/r2.9-builder-discovery-fixture-20261104
      Test fixture implemented with controlled discovery; automated validation remains pending.
- [x] macOS: a menu bar with the app's own actions (open a file or folder, new project, settings,
      Simple / Advanced, Clean room / Deep, the command palette, help); the system's menu bar
      showed only the app menu
      — claimed: sanketdofe, branch feature/r2.9-macos-menu-bar
- [x] A Traditional Chinese README (`README.zh-hant.md`) translated from `README.md` and linked
      from the language switcher in every README
      — claimed: monarchsofchaos004-gif, branch docs/readme-zh-hant; PR #926
- [ ] macOS: an Edit menu (Undo, Cut, Copy, Paste, Select All). The keys work in text boxes, but
      the system's Edit items need the text box to answer them, so they'd show greyed out today
- [x] The Advanced console's prompt box starts tall enough to read what's typed (#942)
      — claimed: justinjohn0306, branch bugfix/942-console-input-height
- [x] A French README (`README.fr.md`) translated from `README.md` and linked from the language
      switcher in every README (#831)
- [ ] Vietnamese letters with stacked marks (ạ ả ấ ế ơ ư ữ…) aren't in the bundled DM Sans or egui's
      default fonts: without craft-fonts they show as boxes, and with it each such letter comes from
      Noto Sans CJK while the rest of the word is DM Sans. Bundle a DM Sans build that has the
      Vietnamese set (with its OFL notice) so Tiếng Việt renders in one face

### R2.10 Pinned builds
Owner: maintainers · Status: in progress
Keep a build that works at hand: pin it, and open it from the sidebar whatever the builder does
to the project next.
- [ ] Say when a light pin ended up copying what it meant to share (a drive without hard links)
      — in PR #220
- [ ] Pins of one project sharing with each other, so a third frozen copy of the same assets
      takes no more space
- [ ] Pin a website (a frozen copy of the built site, each served on its own port)
- [ ] A pin started before the app restarted shows as running (keep its process id and start
      time in the pin, and check both), and can be stopped
- [ ] Cap a run's log (`last-run.log`, `play.log`): it grows as long as the program prints
      — in PR #218
- [ ] Opening a pin that's already running brings its window to the front, instead of a message
- [ ] How much space pins take in Settings

### R2.15 Providers as data
Owner: open · Status: todo
Provider facts live in six places (`BackendKind` arms, `keys::env_var`, `detect::env_keys`,
`PROVIDERS`, `LOCAL_SERVERS`, `AiHub`'s matches); `catalog_id` is never read.
- [ ] One embedded `providers.toml` (versioned) for every built-in provider, local server and
      gateway; `GOOGLE_API_KEY` for Gemini too
- [ ] One build, list and reach path per protocol from the row; a `ProviderAdapter` seam for
      non-Bearer auth; `provider` on each saved backend, filled when read
- [ ] Your own providers in `[[ai.providers]]` (UI, CLI, `ai_provider_add`), settings version 4,
      a JSON schema
- [ ] Settings version 4 lands together with `[[ai.providers]]` (decided: bump with
      `[[ai.providers]]`, not for the `provider` field alone)
- [ ] Model capabilities (context, tools, combo) in pickers and as the default context budget
- [ ] Prices by provider and model (1-hour cache writes, Haiku 5.5's long-prompt tier); gateways and
      subscriptions show unknown or estimated cost
- [ ] Provider-specific remediation table keyed on status and provider code; 402 isn't bad input
- [ ] A cached models.dev catalogue, refreshed only on request (`ai_catalog_refresh`, never at
      start-up; a versioned file; used only for fields that are unknown) (decided: yes, opt-in only)

### R2.16 More providers
Owner: open · Status: todo
- [ ] Cerebras, NVIDIA, Hugging Face, SambaNova, Cohere rows
- [ ] LiteLLM proxy (port 4000, gateway) and Jan (1337); optional keys for LM Studio, llama.cpp, vLLM
- [ ] Z.ai GLM rows
- [ ] Qwen through Alibaba Model Studio (workspace and region in the address)
- [ ] MiniMax, with `reasoning_content` sent back alongside tool calls
- [ ] Azure OpenAI (resource, deployment, `api-key` header)
- [ ] Bedrock with a Bedrock API key; Vertex with a `gcloud` token
- [ ] **decision** AWS SigV4 signing (parked; Bedrock takes its API key as a Bearer key)
- [ ] **decision** A Responses-API protocol (Perplexity's Agent API) (parked)

---

## M3 Engine depth

An open lane: anyone may take these at any time.

### R3.3 Frida sessions for real
Owner: open · Status: todo
- [ ] Replace the in-memory session bookkeeping with real attach, resume and detach

### R3.9 Game assets out
Owner: open · Status: in progress
- [ ] Unity: sounds out (the stored stream, usually FSB5; PCM ones as WAV)
- [ ] Unity: crunched textures (DXT1Crunched, DXT5Crunched; two of the games checked use them)
- [x] Unity: the float formats (RHalf, RGHalf, RGBAHalf, RFloat, RGFloat, RGBAFloat)
      — claimed: Afloat16, branch feature/r3.9-unity-float-textures
- [ ] Unreal 5 and Oodle: IoStore packages, unversioned properties and Oodle-compressed paks
- [ ] Game files: save pictures from IoStore containers (needs the Unreal 5 support above)
- [ ] Unreal: sounds and meshes out (so a Deep-mode rebuild needs no outside converter for them)

### R3.11 Single-file apps, game files and stand-in backends
Owner: open · Status: in progress
- [x] Validate the remaining loopback fixture corrections for complete and fragmented request
      headers, negative probes and stand-in cancellation on the current main
      — claimed: Afloat16, branch test/loopback-fixture-stability-20261103
      Revision `cd468aca`, synchronized with main `41d2175e`, passed
      [full Linux CI](https://github.com/storytold/craft-anything/actions/runs/41765850311).
      The fragmented-header regression withholds the final byte until the next read and keeps
      the existing request and response assertions. The request-encoding, negative-probe,
      cancellation and builder-discovery smoke checks passed in the same run.
- [x] Verify stand-in session cleanup by the original answerer's lifetime rather than polling a
      released port that another listener may reuse
      — claimed: Afloat16, branch test/r3.11-standin-session-lifetime-20261103
- [ ] Stand-in backend: writes that change later reads (recorded answers are replayed as they are)
- [ ] Tell the builder about the stand-in backend in a website's brief, and start one from a
      recording
- [x] Stand-in backend: retain 304 samples without a usable cached body for diagnostics, skip them
      during replay, and report `cached_body_unavailable` when the best matching group has no
      usable answer. Build results count them; cached bodies still answer as 200 (#543)
      — claimed: Afloat16, branch bugfix/r3.11-standin-empty-304-20261103
- [ ] Godot format 4 and IL2CPP metadata 38 and 39 are header-only; read their tables once a real
      file can be checked
- [ ] Bun: take source maps and bytecode out as files; a standalone `.pyz`
- [ ] `native_source_folder_write` uses the built-in decompiler only: use Ghidra in Deep mode when
      it's there
- [x] Keep stand-in response samples distinct when headers, content types or body-availability metadata differ.
      Compare header names without case and ignore ordering between different names, preserving each name's value order.
      At the sample cap, a usable answer can replace an unavailable 304 for the same path, query and request key;
      other requests keep their missing-body evidence. Refused bodies cannot replace that evidence.
      Compare complete samples before charging retained bytes, using #717's retained-budget accounting.
      Historical revision `f79dc0bd` passed [full Linux CI](https://github.com/storytold/craft-anything/actions/runs/41793241151):
      all five original metadata regressions and nine header and capacity regressions passed.
      All 24 combined replay regressions pass locally on the refresh onto `cf414ff4`.
      See this PR's checks for the refreshed revision's complete gate.
      — claimed: Afloat16, branch bugfix/r3.11-standin-sample-metadata-20261103b
- [ ] `native_publisher_show` hashes the file minus the signature fields, not section by section;
      a file with unusual gaps between sections can read as different
- [x] Enforce stand-in per-body and total byte budgets on the text that is actually retained after redaction;
      deduplicate consecutive samples before charging the capture budget.
      — claimed: Afloat16, branch bugfix/r3.11-standin-redacted-budget-20261103b
      Revision `d601bc4e` passed [full Linux CI](https://github.com/storytold/craft-anything/actions/runs/41788735130):
      all four retained-budget regressions and six deduplication regressions passed.

### R3.12 Format parser correctness
Owner: open · Status: in progress
- [x] Require a complete DNS question's type and class fields before retaining it, and keep
      truncated questions out of packet inventories while reporting the incomplete message
      — claimed: Afloat16, branch bugfix/r3.12-dns-question-class-review-20261104
- [x] Reject impossible IPv4 header and total lengths.
      — claimed: Afloat16, branch bugfix/r3.12-ipv4-header-lengths-20261105
- [x] Validate the version in IPv6 frames.
      — claimed: Afloat16, branch bugfix/r3.12-ipv6-version-20261105
- [x] Stop decoding extension headers inside noninitial IPv6 fragments.
      — claimed: Afloat16, branch bugfix/r3.12-ipv6-fragment-payload-20261105
- [x] Enforce the expanded DNS name wire-length limit.
      — claimed: Afloat16, branch bugfix/r3.12-dns-name-wire-length-20261105
- [x] Report truncated DNS TXT strings as incomplete.
      — claimed: Afloat16, branch bugfix/r3.12-dns-txt-truncation-20261105
      Preserve empty ANY/NONE UPDATE prerequisites with zero TTL; keep TXT validation scoped to parsed answers/prerequisites.
      Preserve empty multicast/unicast DNS-SD TXT records using wire-label service instance names.
- [x] Read HTTP Host fields only from the header section.
      — claimed: Afloat16, branch bugfix/r3.12-http-host-boundary-20261105
- [x] Keep capture flow timestamps in chronological order.
      — claimed: Afloat16, branch bugfix/r3.12-capture-flow-time-range-20261105
- [x] Reject incomplete simple packet block payloads.
      — claimed: Afloat16, branch bugfix/r3.12-pcapng-simple-block-length-20261105
      Count rejected lengths against the packet scan limit and summarize them once at the end of the walk.
- [x] Bound rejected packet records, validate timestamp fractions and check pcapng section-local interfaces.
      — claimed: Afloat16, branch bugfix/r3.12-capture-timing-framing
- [x] Keep ZIP directory entries within the declared directory.
      — claimed: Afloat16, branch bugfix/r3.12-zip-directory-bounds-20261105
- [x] Refuse split ZIP archives without their other volumes.
      — claimed: Afloat16, branch bugfix/r3.12-zip-multidisk-20261105
- [x] Check Unity bundle blocks against their declared output lengths.
      — claimed: Afloat16, branch bugfix/r3.12-unity-bundle-decompressed-length-20261105
- [x] Locate Unity bundle metadata within the declared bundle size.
      — claimed: Afloat16, branch bugfix/r3.12-unity-bundle-declared-size-20261105
- [x] Check that Unity bundle compressed blocks end before the trailing block table, or the declared
      bundle end for front-positioned tables, so bytes outside the data region cannot supply block contents.
      — claimed: Codex, branch bugfix/r3.12-unity-bundle-data-region; PR #1021
- [x] Reject Unity serialized objects outside their declared file.
      — claimed: Afloat16, branch bugfix/r3.12-unity-serialized-object-ranges-20261105
- [x] Preserve compression method slots in Unreal pak indexes.
      — claimed: Afloat16, branch bugfix/r3.12-unreal-pak-compression-slots-20261105
- [x] Require complete terminators in Unreal FString values.
      — claimed: Afloat16, branch bugfix/r3.12-unreal-fstring-terminators-20261105
- [x] Count every compression block spanned by an IoStore chunk.
      — claimed: Afloat16, branch bugfix/r3.12-iostore-unaligned-chunk-blocks-20261105
- [x] Reject truncated IoStore fixed headers.
      — claimed: Afloat16, branch bugfix/r3.12-iostore-complete-header-20261105
- [x] Reject invalid marshal long integer digits.
      — claimed: Afloat16, branch bugfix/r3.12-pyc-long-limb-validation-20261105
- [x] Check expanded RGBA image sizes and enforce output budgets before allocation.
      — claimed: Afloat16, branch bugfix/r3.12-pixel-rgba-output-overflow-20261105
- [x] Count empty tensor shapes consistently across axis orders.
      — claimed: Afloat16, branch bugfix/r3.12-tensor-zero-dimension-product-20261105
- [x] Find APK signatures through valid ZIP comments and framed central directories.
      — claimed: Afloat16, branch bugfix/r3.12-apk-sign-eocd-comment-boundary-20261105
- [x] Keep Mach-O load-command facts within their command.
      — claimed: Afloat16, branch bugfix/r3.12-macho-load-command-framing-20261105
- [x] Keep Mach-O function starts within their declared payload.
      — claimed: Afloat16, branch bugfix/r3.12-macho-function-starts-payload-boundary-20261105
- [x] Keep code-signature children and verification reads within their declared superblob.
      — claimed: Afloat16, branch bugfix/r3.12-codesign-superblob-length-boundary-20261105
- [x] Keep PE optional-header reads within the declared header.
      — claimed: Afloat16, branch bugfix/r3.12-pe-optional-header-framing-20261105
- [x] Keep PE resource metadata within the resource directory.
      — claimed: Afloat16, branch bugfix/r3.12-pe-resource-directory-boundary-20261105
- [x] Keep Go function tables and names within their section.
      — claimed: Afloat16, branch bugfix/r3.12-go-pclntab-section-boundary-20261105
- [x] Preserve complete Unicode characters in shortened .NET strings.
      — claimed: Afloat16, branch bugfix/r3.12-dotnet-user-string-surrogate-limit-20261105
- [x] Locate ELF core mapping names after the complete declared table.
      — claimed: Afloat16, branch bugfix/r3.12-elf-core-file-name-count-cap-20261105
- [x] Keep DOS relocations inside the declared executable header.
      — claimed: Afloat16, branch bugfix/r3.12-dos-relocation-header-boundary-20261105
- [x] Parsers refuse what they used to half-read: binary plists keep objects inside their object
      area and reject unrepresentable widths, base64 (plists, source maps) refuses damaged tails
      and incomplete quanta through one shared decoder, OIDs must be minimally encoded, and string
      offsets past the address space are left out. Tar keeps old GNU timestamps out of names and
      PAX sizes across other metadata records; CRC cpio checks member data; source maps decode
      percent-escaped data URLs and report indexed-map columns in the generated file
      (#606–#615, by Afloat16)
- [x] Bound and cancel CRC cpio verification across firmware probes, and reject non-hex checksum
      fields — claimed: Afloat16, branch bugfix/r3.12-bound-cpio-crc-scans-20261103
- [x] More parsers keep to the bytes they declare and refuse impossible values: signed LEB128 past
      64 bits, damaged modified UTF-8 (valid text around it survives), widened Java instructions
      and invocation operands, DEX register lists and array widths, SQLite text in the database's
      encoding with cells kept inside their page and overflow cycles stopped, Android string pools,
      ZIP output limits for stored entries, decoded sizes and ZIP64 fields, ASAR header framing,
      ROMFS dot roots, cramfs, ext and JFFS2 sizes and holes, PyInstaller modules outside their
      PYZ, JSON config edits (comments kept, every duplicate key removed), and UDP, TCP, DNS over
      TCP, pcapng and TLS lengths (#688–#715, by Afloat16)
- [x] Check ClientHello vector lengths and extension values before reporting the handshake as complete.
      — claimed: Afloat16, branch bugfix/r3.12-tls-client-hello-values

- [x] Address the combined parser review: preserve partial Java reference coverage,
      enforce DEX signed 32-bit LEB128 limits, recover legacy AAPT UTF-8 lengths within
      their pool with a shared recovery probe limit, and distinguish missing ext indirect
      metadata from sparse holes.
      — claimed: Afloat16, branch bugfix/r3.12-parser-review-followups-main-20261104
      Shared-budget revision `29c48e03` passed [full Linux CI](https://github.com/storytold/craft-anything/actions/runs/41762780268).
      The merge with main `41d2175e` awaits a new CI run.

- [x] Keep DNS answer names inside their declared RDATA, including compressed names and MX
      exchanges, and report malformed records as incomplete instead of inventing answer values,
      while preserving legal empty DNS UPDATE RRset prerequisites
      — claimed: Afloat16, branch bugfix/r3.12-dns-rdata-names-review-20261104
- [x] Keep address and service records attached to their owner and validate the record's wire fields.
      — claimed: Afloat16, branch bugfix/r3.12-dns-record-evidence

### R3.13 Web inspection correctness
Owner: open · Status: in progress
- [x] The HTML scanner finds script preloads in `rel` token lists, decodes asset URLs once (numeric
      and named references), keeps script and title text through lookalike closing tags, and skips
      raw-text elements, markup inside attribute values and custom tag names such as
      `<script:widget>` (#591–#596, by Afloat16)
- [x] HAR reports redact credentials whose query names are percent-encoded (and the stand-in
      backend matches older recordings the same way), redact declared secrets in every kept field
      (names, methods, hosts, versions, timings), count body bytes before text conversion, and
      template only real calendar dates in API paths (#602–#605, by Afloat16)
- [x] Preserve empty query pairs when matching older stand-in recordings, without treating raw
      query separators as pairs — claimed: Afloat16, branch `bugfix/r3.13-preserve-empty-query-pairs-20261103`

---

## M4 Release

### R4.1 A macOS CLI that opens
Owner: maintainers · Status: todo
- [ ] **decision** A standalone macOS CLI that can carry its ticket, such as a stapled `.pkg`; that
      needs a Developer ID Installer certificate in the release secrets

### R4.2 Managed tier
Owner: maintainers · Status: todo
- [ ] **decision** Managed tier: account backend, usage metering (Claude API under our own key only)
- [ ] Before the managed tier or any paid offering: confirm the Claude Code orchestration question
      with Anthropic (claude-code.md)

---

## Parked

Looked at and not committed to; none of this is open work. Say in the issue if you want one.

- From the tool survey: firmware image kinds (ESP32, U-Boot, device trees, Intel HEX), Erlang BEAM,
  GPU code objects, kernel modules, GameCube DOL, Mac crash reports, SAZ and qlog captures, protobuf
  bodies, R8 mappings, JNI links, MCP over HTTP
- Ideas from issues: resume by itself after a usage limit (#476, the one tied to a real problem), a
  source folder as a target (#480), actions after a build ends (#495), WebAssembly disassembly
  (#175), an HTTPS intercepting proxy (#164), VM and Windows Sandbox detonation (#144, #163), an
  adapter for another disassembler (#147)
- More agents (#14, #63, #97, #162, #170, #186, #206, #470): R2.2's table comes first; providers
  moved to R2.15/R2.16
