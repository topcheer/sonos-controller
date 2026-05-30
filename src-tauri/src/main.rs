#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    sonos_controller_lib::run();
}
