# Phase 0 spike – findings (Claude Code 2.1.310, Linux)

Throwaway Rust program that drives `claude` headless. Run it with:

```
cargo run -- run <cwd> "<prompt>" [session_id]
cargo run -- cancel-test <cwd> <group|term|tree>
```
`CLAUDE_BIN` and `CLAUDE_MODEL` (default `haiku`) can be set in the environment.

## What works

| Topic | Result |
|---|---|
| Headless run | `claude -p --input-format stream-json --output-format stream-json --verbose --permission-mode bypassPermissions`. Write one `{"type":"user",...}` line to stdin. The CLI waits for more input after `result`, so **close stdin after `result`** and the process exits with 0. |
| Events seen | `system/init` (session_id, tools), `assistant` (content blocks: `text`, `thinking`, `tool_use`), `user` (tool results), `system/thinking_tokens`, `system/task_summary`, `rate_limit_event`, `system/post_turn_summary`, `result`. Unknown types must be tolerated. |
| Resume | `--resume <session_id>` with the answer as the new user message continues the same session (same id); the agent finished the task. |
| `ask_user` tool | Works as planned. The app binary itself is the MCP server (`agent-spike mcp-ask-user`, ~50 lines of JSON-RPC over stdio, **stateless**). The app reads the question from the agent's `tool_use` event, so the MCP server and the app don't need to talk to each other. Passed with `--strict-mcp-config --mcp-config '<json>'` (ignores the user's own MCP servers). The agent called it, ended its turn, ticket → Needs Input. Note: MCP tools are loaded on demand, so the agent first calls `ToolSearch`; that's fine. |
| Cost | Every test turn with haiku cost < $0.01. |

## Surprises (good)

1. **`system/post_turn_summary`**: after every turn the CLI reports `status_category` (`completed` / `blocked` / …) and `needs_action` (e.g. *"choose the file name: a.txt or b.txt"*). In a test without the tool, a plain-text question came out as `blocked`. **This is a second Needs Input signal for free** and covers the case where the agent ignores `ask_user`.
2. **`rate_limit_event`**: reports `unifiedWindows.five_hour.utilization` and `seven_day.utilization` (0.0–1.0) plus `resetsAt`. **The "% usage per session and week" from the original plan is available after all**, at least for subscription logins.

Both look undocumented → use them, but parse defensively and treat them as optional.

## Surprise (bad): cancel

Claude Code starts **each shell command in its own session** (`setsid`: SID = PGID = shell pid). So the usual "kill our process group" approach (`command-group`'s `kill()`) **leaves the agent's running shell commands alive as orphans**.

| Stop mode | Result |
|---|---|
| `group`: SIGKILL to our process group | ❌ the agent's `zsh` + `python3` keep running |
| `term`: SIGTERM to `claude` only | ✅ Claude kills its own shells |
| `tree`: SIGKILL to every descendant (snapshot via ppid) | ✅ |

**Decision:** cancel = SIGTERM → wait ~3 s → snapshot descendants and SIGKILL them all → kill the group.
In the app, consider also setting `PR_SET_CHILD_SUBREAPER` on Linux, so orphans attach to the app and can't escape.

Also: Claude Code refuses long foreground `sleep N` commands, so tests need another long-running command.

## Not yet verified

- **Windows** (no Windows machine here): binary lookup (`claude.exe` vs `claude.cmd`), Git Bash requirement, and whether a Job Object catches the agent's shells (it should, unless they break away from the job). The same spike should run there; `signal()`/`descendants()` need Windows versions.
- OpenCode backend (`opencode serve`).
- How reliably the agent uses `ask_user` with bigger models and longer tasks.
