//! Minimal stdio MCP server exposing the `ask_user` tool to Claude Code.
//!
//! Claude starts it as `hivemind mcp-ask-user`. It is stateless: the agent manager sees the
//! question in the agent's own `tool_use` event, so the tool only acknowledges it and tells
//! the agent to end its turn. The answer arrives later as the next prompt (`--resume`).

use std::io::{BufRead, Write};

use serde_json::{Value, json};

pub const SERVER_NAME: &str = "hivemind";
pub const TOOL_NAME: &str = "ask_user";
/// Name under which Claude Code exposes the tool to the model.
pub const QUALIFIED_TOOL_NAME: &str = "mcp__hivemind__ask_user";
pub const SUBCOMMAND: &str = "mcp-ask-user";

pub fn serve() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if let Some(response) = handle(&line)
            && writeln!(stdout, "{response}").and_then(|_| stdout.flush()).is_err()
        {
            break;
        }
    }
}

/// Answers one JSON-RPC message; notifications (no `id`) get no answer.
fn handle(line: &str) -> Option<Value> {
    let req: Value = serde_json::from_str(line).ok()?;
    let id = req.get("id")?.clone();
    let result = match req["method"].as_str().unwrap_or_default() {
        "initialize" => json!({
            "protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": [{
            "name": TOOL_NAME,
            "description": "Ask the user a question or for a decision. After calling this, end your \
                            turn immediately; the answer arrives as the next user message.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "question": { "type": "string", "description": "The question for the user." },
                    "options": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional answer choices.",
                    },
                },
                "required": ["question"],
            },
        }]}),
        "tools/call" => json!({ "content": [{
            "type": "text",
            "text": "Question delivered to the user. End your turn now without further text; \
                     the answer will arrive as the next message.",
        }]}),
        _ => {
            return Some(json!({
                "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "method not found" },
            }));
        }
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_requests_and_ignores_notifications() {
        let init =
            handle(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"x"}}"#);
        assert_eq!(init.unwrap()["result"]["protocolVersion"], "x");
        assert!(handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        let list = handle(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        assert_eq!(list["result"]["tools"][0]["name"], TOOL_NAME);
        let unknown = handle(r#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#).unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }
}
