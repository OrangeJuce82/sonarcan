#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    sonarcan_lib::run();
}
