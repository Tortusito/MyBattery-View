#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod battery;
mod settings;

use battery::{collect_all_devices, BatteryDevice};
use settings::{load_settings, save_settings, AppSettings};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, State, WindowEvent};

struct AppState {
    devices: Arc<Mutex<Vec<BatteryDevice>>>,
    settings: Arc<Mutex<AppSettings>>,
}

#[tauri::command]
fn get_devices(state: State<AppState>) -> Vec<BatteryDevice> {
    state.devices.lock().map(|d| d.clone()).unwrap_or_default()
}

#[tauri::command]
fn refresh_devices(app: AppHandle, state: State<AppState>) -> Vec<BatteryDevice> {
    let list = collect_all_devices();
    if let Ok(mut guard) = state.devices.lock() {
        *guard = list.clone();
    }
    update_tray_tooltip(&app, &list);
    let _ = app.emit("devices-updated", &list);
    list
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> AppSettings {
    state.settings.lock().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
fn save_app_settings(app: AppHandle, state: State<AppState>, new_settings: AppSettings) -> AppSettings {
    save_settings(&new_settings);
    if let Ok(mut guard) = state.settings.lock() {
        *guard = new_settings.clone();
    }
    let _ = app.emit("settings-updated", &new_settings);
    new_settings
}

#[tauri::command]
fn set_device_icon(app: AppHandle, state: State<AppState>, id: String, icon: String) -> AppSettings {
    let updated = {
        let mut guard = state.settings.lock().unwrap();
        if icon.is_empty() {
            guard.custom_icons.remove(&id);
        } else {
            guard.custom_icons.insert(id, icon);
        }
        save_settings(&guard);
        guard.clone()
    };
    let _ = app.emit("settings-updated", &updated);
    updated
}

#[tauri::command]
fn set_device_name(app: AppHandle, state: State<AppState>, id: String, name: String) -> AppSettings {
    let updated = {
        let mut guard = state.settings.lock().unwrap();
        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            guard.custom_names.remove(&id);
        } else {
            guard.custom_names.insert(id, trimmed);
        }
        save_settings(&guard);
        guard.clone()
    };
    let _ = app.emit("settings-updated", &updated);
    updated
}

#[tauri::command]
fn toggle_hide_device(app: AppHandle, state: State<AppState>, id: String) -> AppSettings {
    let updated = {
        let mut guard = state.settings.lock().unwrap();
        if let Some(pos) = guard.hidden_devices.iter().position(|x| x == &id) {
            guard.hidden_devices.remove(pos);
        } else {
            guard.hidden_devices.push(id);
        }
        save_settings(&guard);
        guard.clone()
    };
    let _ = app.emit("settings-updated", &updated);
    updated
}

#[tauri::command]
fn clear_hidden_devices(app: AppHandle, state: State<AppState>) -> AppSettings {
    let updated = {
        let mut guard = state.settings.lock().unwrap();
        guard.hidden_devices.clear();
        save_settings(&guard);
        guard.clone()
    };
    let _ = app.emit("settings-updated", &updated);
    updated
}

#[tauri::command]
fn open_external(url: String) {
    if !url.starts_with("https://github.com/Tortusito/MyBattery-View") {
        return;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
}

#[tauri::command]
fn open_main_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
    if let Some(flyout) = app.get_webview_window("flyout") {
        let _ = flyout.hide();
    }
}

#[tauri::command]
fn hide_flyout(app: AppHandle) {
    if let Some(flyout) = app.get_webview_window("flyout") {
        let _ = flyout.hide();
    }
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn update_tray_tooltip(app: &AppHandle, devices: &[BatteryDevice]) {
    let text = if devices.is_empty() {
        "MyBattery-View · Sin dispositivos".to_string()
    } else {
        let summary: String = devices
            .iter()
            .map(|d| format!("{}: {}%", d.name, d.percent))
            .collect::<Vec<_>>()
            .join(" · ");
        if summary.chars().count() > 120 {
            let truncated: String = summary.chars().take(117).collect();
            format!("{}...", truncated)
        } else {
            summary
        }
    };

    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(&text));
    }
}

fn toggle_flyout(app: &AppHandle, device_count: usize, pos_x: f64, pos_y: f64) {
    let Some(flyout) = app.get_webview_window("flyout") else {
        return;
    };

    if flyout.is_visible().unwrap_or(false) {
        let _ = flyout.hide();
        return;
    }

    let rows = device_count.max(1).min(6) as i32;
    let height = 84 + rows * 48;
    let width = 310;
    let _ = flyout.set_size(Size::Physical(PhysicalSize::new(width as u32, height as u32)));

    let mut x = (pos_x as i32) - (width / 2);
    let mut y = (pos_y as i32) - height - 12;

    if let Ok(Some(monitor)) = flyout.current_monitor() {
        let m_pos = monitor.position();
        let m_size = monitor.size();
        let max_x = m_pos.x + (m_size.width as i32) - width - 12;
        x = x.clamp(m_pos.x + 12, max_x.max(m_pos.x + 12));
        if y < m_pos.y + 12 {
            y = (pos_y as i32) + 12;
        }
    }

    let _ = flyout.set_position(Position::Physical(PhysicalPosition::new(x, y)));
    let _ = flyout.show();
    let _ = flyout.set_focus();
}

fn main() {
    let initial_settings = load_settings();
    let devices_arc = Arc::new(Mutex::new(Vec::<BatteryDevice>::new()));
    let settings_arc = Arc::new(Mutex::new(initial_settings));

    tauri::Builder::default()
        .manage(AppState {
            devices: devices_arc.clone(),
            settings: settings_arc.clone(),
        })
        .invoke_handler(tauri::generate_handler![
            get_devices,
            refresh_devices,
            get_settings,
            save_app_settings,
            set_device_icon,
            set_device_name,
            toggle_hide_device,
            clear_hidden_devices,
            open_external,
            open_main_window,
            hide_flyout,
            quit_app,
        ])
        .setup(move |app| {
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(main_win) = app.get_webview_window("main") {
                    let _ = main_win.hide();
                }
            }
            let open_item = MenuItem::with_id(app, "open", "Abrir MyBattery-View", true, None::<&str>)?;
            let refresh_item = MenuItem::with_id(app, "refresh", "Actualizar ahora", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &refresh_item, &quit_item])?;

            let tray_icon = Image::from_bytes(include_bytes!("../icons/tray.png"))?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => open_main_window(app.clone()),
                    "refresh" => {
                        let state = app.state::<AppState>();
                        let list = collect_all_devices();
                        if let Ok(mut guard) = state.devices.lock() {
                            *guard = list.clone();
                        }
                        update_tray_tooltip(app, &list);
                        let _ = app.emit("devices-updated", &list);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        let count = app
                            .state::<AppState>()
                            .devices
                            .lock()
                            .map(|d| d.len())
                            .unwrap_or(0);
                        toggle_flyout(app, count, position.x, position.y);
                    }
                })
                .build(app)?;

            update_tray_tooltip(app.handle(), &[]);

            if let Some(flyout) = app.get_webview_window("flyout") {
                let flyout_clone = flyout.clone();
                flyout.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = flyout_clone.hide();
                    }
                });
            }

            let app_handle = app.handle().clone();
            std::thread::spawn(move || loop {
                let list = collect_all_devices();
                if let Ok(mut guard) = app_handle.state::<AppState>().devices.lock() {
                    *guard = list.clone();
                }
                update_tray_tooltip(&app_handle, &list);
                let _ = app_handle.emit("devices-updated", &list);

                let interval = app_handle
                    .state::<AppState>()
                    .settings
                    .lock()
                    .map(|s| s.poll_interval_sec.max(10))
                    .unwrap_or(30);
                std::thread::sleep(Duration::from_secs(interval));
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running MyBattery-View");
}
