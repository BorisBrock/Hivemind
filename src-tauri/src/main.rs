// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if !hivemind_lib::run_mcp_server_if_requested() {
        hivemind_lib::run()
    }
}
