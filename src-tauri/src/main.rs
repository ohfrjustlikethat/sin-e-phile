// Release builds must not spawn a console window behind the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    sin_e_phile_lib::run()
}
