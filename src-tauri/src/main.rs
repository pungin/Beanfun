// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(code) = beanfun_lib::services::updater::install::run_helper_if_requested() {
        std::process::exit(code);
    }
    beanfun_lib::run()
}
