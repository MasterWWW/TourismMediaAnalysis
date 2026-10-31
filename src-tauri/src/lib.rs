pub mod apply;
pub mod commands;
pub mod date;
pub mod dedup;
pub mod events;
pub mod journal;
pub mod logging;
pub mod models;
pub mod plan;
pub mod report;
pub mod scan;
pub mod state;
pub mod tools;

use state::AppState;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _logging_guard = logging::init();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        platform = std::env::consts::OS,
        "starting tourism media organizer"
    );
    tauri::Builder::default()
        .manage(AppState::new())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::scan,
            commands::cancel_scan,
            commands::get_scan_status,
            commands::get_inventory,
            commands::get_plan_summary,
            commands::get_plan_page,
            commands::set_item_selected,
            commands::set_all_selected,
            commands::apply,
            commands::get_apply_result,
            commands::rollback,
            commands::delete_quarantine,
            commands::get_tool_status,
            commands::set_tool_paths
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
