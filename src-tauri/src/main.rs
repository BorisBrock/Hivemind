// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if !workzone_lib::run_mcp_server_if_requested() {
        workzone_lib::run()
    }
}
