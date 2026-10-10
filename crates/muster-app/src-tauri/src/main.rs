// Không mở cửa sổ console đen trên Windows ở bản release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    muster_app_lib::run();
}
