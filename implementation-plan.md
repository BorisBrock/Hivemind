# Workzone – Implementation Plan

Derived from [project-plan.md](project-plan.md).

## 1. Recommended tech stack

| Layer | Choice | Why |
|---|---|---|
| App shell | **Tauri 2** | Rust core, small binaries, native windows. The UI is a web view, which makes a polished markdown editor, themes and rich transcript rendering much easier than native Rust GUIs. |
| Core / backend | **Rust** (tokio, serde, `tauri::ipc::Channel` for streaming) | Process supervision, the per-project queue, persistence. Fast and robust. |
| Persistence | **SQLite** via `sqlx` (with migrations) | Projects, tickets, transcript events, settings. One file, transactional, no server. |
| Frontend | **Svelte 5 + TypeScript + Vite** | Small, fast, little boilerplate. (React works too if you prefer its ecosystem.) |
| Styling / themes | **Tailwind CSS v4 + CSS custom properties** | Each theme is a set of CSS variables; project colours come from a palette of variables. |
| Markdown editor | **CodeMirror 6** with markdown mode + live preview (or **Milkdown** for full WYSIWYG) | Mature, fast, can be themed. |
| Markdown rendering | `markdown-it` + **Shiki** for code blocks | Rendering the agent's output and diffs. |
| Drag & drop | `svelte-dnd-action` | Kanban columns. |
| Agent integration | Spawn the user's installed **`claude` CLI** in headless stream-JSON mode (`claude -p --input-format stream-json --output-format stream-json --verbose`, `--resume <session_id>`); OpenCode through `opencode serve` (HTTP + SSE) | Both are documented, machine-readable interfaces. The agent runs under the user's own CLI and login (see blocker B3). |

**Why not iced/egui/Slint?** They are fast, but a "beautiful markdown editor" and a rich, scrollable transcript with code highlighting would take a lot of custom widget work. Tauri gives you the web ecosystem for the UI and Rust where it matters.

## 2. Architecture

```
┌──────────────── Frontend (Svelte, WebView) ────────────────┐
│ ProjectSidebar │ KanbanBoard │ TicketView │ Settings │ StatusBar
└──────────────▲───────────── IPC (commands + Channels) ─────┘
               │
┌──────────────┴──────────── Rust core ───────────────────────┐
│ Store (SQLite)   Scheduler (1 active ticket per project)    │
│ AgentManager ── trait AgentBackend                          │
│                   ├─ ClaudeCodeBackend (CLI, stream-json)   │
│                   └─ OpenCodeBackend  (HTTP/SSE)            │
│ AskUserTool (MCP, non-waiting → "Needs Input" in the UI)    │
│ UsageTracker                                                │
└─────────────────────────────────────────────────────────────┘
```

### Core domain model
- **Project**: id, name, path, color, hidden, sort_order, agent_override?
- **Ticket**: id, project_id, title, body (prompt), status, position (rank within its column), agent_session_id?, created/updated timestamps
- **TicketEvent** (append-only transcript): ticket_id, seq, kind (`user_prompt | assistant_text | thinking | tool_use | tool_result | permission_request | question | result | error | system`), payload JSON, timestamp
- **Settings**: theme, palette, default agent, agent binary paths

### Ticket state machine
```
Planning ──user──▶ Ready ──scheduler──▶ InProgress ──agent asks──▶ NeedsInput
   ▲                 ▲                     │  ▲                          │
   │                 │                     │  └──────user answers────────┘
   │                 └──user comment───────┤
   └──cancel (user)────────────────────────┤
                                           ▼
                                         Done ──user close──▶ deleted
```
- The body can only be edited in **Planning**. After that, the ticket view shows the transcript and an input box.
- "Send back to Ready with a comment" resumes the **same** agent session (`--resume`), with the comment as the next prompt.
- **Scheduler**: per project, take the top-ranked Ready ticket only when no other ticket of that project is In Progress or Needs Input. Needs Input still counts as active, so a second run can't change the same files while the first waits.

## 3. Phases

### Phase 0 – Technical spikes (validate the risky parts first, about 2–3 days)
1. Rust spawns `claude -p` in bidirectional stream-JSON mode, parses every message type, sends a follow-up user message, kills the process, resumes with `--resume`.
2. Run the same spike on **Windows**: find the binary, spawn it with `bypassPermissions`, kill the process tree on cancel.
3. Non-waiting `ask_user` MCP tool: the agent calls it, ends its turn, and the conversation resumes with the answer (see B2). Measure how often the agent uses it instead of asking in plain text.
4. Run `opencode serve` and map its event stream onto the same event kinds.

**Exit criterion:** a headless Rust test program that runs a ticket from start to finish on Linux and Windows, including one `ask_user` round trip and a cancel.

### Phase 1 – Skeleton ✅
*Done. Rust is pinned to 1.99.1 (`rust-toolchain.toml`) because specta needs ≥ 1.93. `@tauri-apps/api` is pinned to 2.11 to match the newest 2.x `tauri` crate.*
- Tauri 2 + Svelte + Tailwind scaffold, lint/format, CI (build and test on Linux and Windows).
- SQLite schema + migrations, Rust repository layer, typed IPC commands (generate TS types with `specta`/`tauri-specta`).
- Theme system (CSS variables, light/dark + 2–3 themes), app layout: sidebar, board, status bar.

### Phase 2 – Projects & tickets (no AI yet) ✅
*Done. Moves are stored relative to the neighbour ticket (`before_id`), so ordering stays correct when hidden projects share a column. Project folders must exist.*
- Project CRUD (folder picker, colour picker from the palette), hide/show toggle, reordering.
- Kanban board with five columns, drag & drop limited to the transitions the user is allowed to make, tickets in project colours.
- Ticket view: CodeMirror markdown editor in Planning, read-only transcript afterwards.

### Phase 3 – Claude Code agent integration (MVP core)
- `AgentBackend` trait: `start(ticket) / send(input) / answer_permission(id, decision) / cancel() / events() -> Stream<TicketEvent>`.
- `ClaudeCodeBackend` built on the Phase 0 spike; persist every event as it arrives, stream it to the UI over a Channel.
- Scheduler + state transitions, activity indicator per project.
- Transcript renderer: assistant markdown, collapsible thinking/tool calls, diffs for edit tools, a clear "agent is asking" block with answer controls.
- Cancel = kill the process tree, ticket goes back to Planning; the next run starts a fresh session.
- Close = delete ticket + transcript (with confirmation).
- **Crash recovery:** on start-up, tickets left In Progress become "Interrupted" (shown in Needs Input) with a Resume button.

### Phase 4 – Settings & status bar
- Settings dialog: theme, palette, default agent, binary paths (with a notice that agents run fully autonomous).
- Status bar: running sessions, tokens/cost per session and per week (from `result` messages).

### Phase 5 – OpenCode backend
- `OpenCodeBackend` via `opencode serve` + SSE, mapped onto the same events and states.

### Phase 6 – Polish
- Keyboard shortcuts, search/filter, notifications when a ticket reaches Needs Input/Done, packaging (AppImage/deb for Linux, NSIS/MSI for Windows), auto-update.

### Later / nice to have
- Git worktree per ticket (would allow several tickets per project at once), showing the diff/branch of a ticket, per-ticket model choice, ticket templates.

## 4. Blockers & risks

| # | Issue | Severity | Mitigation |
|---|---|---|---|
| ~~B1~~ | **"% token usage per session and week" has no official API.** *Decided:* plan-limit % is dropped (it was only a nice-to-have). The status bar shows token counts and cost per session and per week, summed locally. *Spike update:* the stream contains an undocumented `rate_limit_event` with 5-hour and 7-day utilization, so the % can be shown as an optional extra after all. | Resolved | Parse defensively; hide when missing. |
| ~~B2~~ | **Detecting "Needs Input".** *Decided:* (1) by default every end of turn goes to **Done**, where the user can reply (resume the same session) or close. (2) On top of that, a **non-waiting** `ask_user(question, options?)` MCP tool, passed with `--mcp-config` and announced with `--append-system-prompt`. It stores the question, moves the ticket to Needs Input and returns right away with "question delivered, end your turn". The user's answer is sent through `--resume`, so there are no tool timeouts and no state lost on restart. If the model asks in plain text anyway, the ticket lands in Done, which is harmless. | Resolved | *Validated in Phase 0.* Second signal: the CLI's `post_turn_summary` event (`status_category: "blocked"`) also routes plain-text questions to Needs Input. |
| **B3** | **Auth / terms.** Anthropic does not allow third-party apps to offer claude.ai (Pro/Max) login. Embedding the Agent SDK with subscription login is therefore a problem; driving the user's own installed `claude` CLI is the safer path. | Medium | Spawn the CLI and never handle credentials. Check the current terms before distributing the app publicly. |
| ~~B4~~ | **Permissions / safety.** *Decided:* fully autonomous (see Decisions). No permission prompts, no permission bridge. | Resolved | Recommend using git in projects so agent changes can be reviewed or reverted. |
| **B8** | **Windows specifics.** (a) Killing an agent must kill its whole process tree: Job Objects on Windows. On Linux, process groups are **not** enough, because Claude starts each shell command with `setsid` (see `spikes/agent-spike/FINDINGS.md`). Use SIGTERM → wait → kill every descendant. (b) The `claude` command may be `claude.exe` or an npm shim `claude.cmd`, which can't be spawned directly. (c) Claude Code on Windows needs Git for Windows (Git Bash) for its shell tool. (d) Path and line-ending differences. | Medium | Resolve the agent binary at start-up and let the user override the path in Settings. Check for Git Bash and show a clear error. Use a cross-platform process helper (e.g. `command-group` crate). Test on Windows from Phase 0 on. |
| **B5** | **Different CLIs, different models.** Claude Code and OpenCode differ in protocol, session model and permission handling; the stream-JSON format can change between CLI versions. | Medium | Thin `AgentBackend` trait, tolerant parsing (keep unknown events as raw `system` events), pin/check the CLI version at start-up. |
| **B6** | **Linux WebView.** Tauri on Linux uses WebKitGTK, which is slower than Chromium and has known rendering issues on some Wayland/NVIDIA setups. You're on CachyOS, so you'll see this first. | Low–Medium | Virtualize long transcripts, avoid heavy CSS effects, test early on your machine. |
| **B7** | **App restart while agents run.** Child processes die with the app (or turn into orphans). | Low | Crash recovery via `--resume` (Phase 3). Background daemon mode only if this is ever needed. |

## 5. Decisions

1. **Auto-pickup:** moving a ticket to Ready is enough. The scheduler starts it automatically as soon as the project has no active ticket (top of the Ready column first).
2. **Cancel:** the agent process is killed and the ticket goes back to **Planning**. The body is editable again; the next start is a **fresh** agent session. Files the cancelled run already changed stay as they are (no automatic rollback).
3. **Close = delete:** the ticket and its transcript are permanently deleted, after a confirmation dialog. No archive.
4. **Permissions: fully autonomous.** Claude runs with `--permission-mode bypassPermissions`, OpenCode with all permissions set to `allow`. This removes the need for routing permission requests to the UI. Note: the agent can then do anything your user account can, including outside the project folder. The settings dialog shows this clearly.
5. **Platforms: Windows + Linux from day one**, macOS later.
