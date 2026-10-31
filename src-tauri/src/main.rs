// Windows 下 release 构建不弹出控制台窗口；非 Windows 平台保持不变。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tourism_media_organizer_lib::run()
}
