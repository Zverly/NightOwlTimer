mod action;
mod models;
mod storage;
mod windows_task;

use models::{AppData, HistoryRecord, ReminderSettings, ScheduleInfo, ScheduleRequest};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
fn get_history(app: tauri::AppHandle) -> Result<Vec<HistoryRecord>, String> {
    Ok(storage::load(&app)?.history)
}

#[tauri::command]
fn get_active_schedule(app: tauri::AppHandle) -> Result<Option<ScheduleInfo>, String> {
    Ok(storage::load(&app)?.active_schedule)
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Result<AppData, String> {
    storage::load(&app)
}

fn normalized_silent_startup(launch_at_startup: bool, silent_startup: bool) -> bool {
    launch_at_startup && silent_startup
}

fn configure_autostart<Enable, Disable>(
    enabled: bool,
    enable: Enable,
    disable: Disable,
) -> Result<(), String>
where
    Enable: FnOnce() -> Result<(), String>,
    Disable: FnOnce() -> Result<(), String>,
{
    if enabled {
        enable()
    } else {
        disable()
    }
}

#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    reminders: ReminderSettings,
    launch_at_startup: bool,
    silent_startup: bool,
    exit_to_tray: bool,
) -> Result<(), String> {
    let previous_data = storage::load(&app)?;
    let mut next_data = previous_data.clone();
    next_data.reminders = reminders;
    next_data.launch_at_startup = launch_at_startup;
    next_data.silent_startup = normalized_silent_startup(launch_at_startup, silent_startup);
    next_data.exit_to_tray = exit_to_tray;
    storage::save(&app, &next_data)?;

    let autolaunch = app.autolaunch();
    if let Err(error) = configure_autostart(
        launch_at_startup,
        || autolaunch.enable().map_err(|error| error.to_string()),
        || autolaunch.disable().map_err(|error| error.to_string()),
    ) {
        return match storage::save(&app, &previous_data) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!("{error}；同时恢复原设置失败：{rollback_error}")),
        };
    }

    Ok(())
}

#[tauri::command]
fn get_diagnostics(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let data = storage::load(&app)?;
    Ok(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "history_count": data.history.len(),
        "data_directory": storage::data_directory().display().to_string(),
        "data_file": storage::data_file_path().display().to_string()
    }))
}

#[tauri::command]
fn minimize_window(app: tauri::AppHandle) -> Result<(), String> {
    app.get_webview_window("main")
        .ok_or_else(|| "main window is unavailable".to_string())?
        .minimize()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) -> Result<(), String> {
    app.get_webview_window("main")
        .ok_or_else(|| "main window is unavailable".to_string())?
        .hide()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn exit_application(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn open_repository() -> Result<(), String> {
    open::that("https://github.com/Zverly/NightOwlTimer").map_err(|error| error.to_string())
}

#[tauri::command]
fn open_data_file() -> Result<(), String> {
    let path = storage::data_file_path();
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

struct AppState(Mutex<Option<ScheduleInfo>>);

#[derive(Clone, Serialize)]
struct TrayQuickSchedule {
    action: models::Action,
    minutes: i64,
}

struct TrayControls {
    status: tauri::menu::MenuItem<tauri::Wry>,
    plus: tauri::menu::MenuItem<tauri::Wry>,
    minus: tauri::menu::MenuItem<tauri::Wry>,
    cancel: tauri::menu::MenuItem<tauri::Wry>,
    keep: tauri::menu::MenuItem<tauri::Wry>,
}

fn tray_status_label(schedule: Option<&ScheduleInfo>) -> String {
    match schedule {
        None => "当前无计划".into(),
        Some(schedule) => {
            let action = match schedule.action {
                models::Action::Shutdown => "正常关机",
                models::Action::Force => "强制关机",
                models::Action::Sleep => "睡眠",
            };
            format!("{action} · {}", &schedule.target_time_local[..16])
        }
    }
}

fn history_detail(status: &str, target_time: &str) -> String {
    format!("{status} · {target_time}")
}

fn parse_quick_schedule_id(id: &str) -> Option<(models::Action, i64)> {
    let (action, minutes) = id.strip_prefix("quick-")?.rsplit_once('-')?;
    let action = match action {
        "shutdown" => models::Action::Shutdown,
        "force" => models::Action::Force,
        "sleep" => models::Action::Sleep,
        _ => return None,
    };
    let minutes = match minutes {
        "30" => 30,
        "60" => 60,
        "90" => 90,
        "120" => 120,
        _ => return None,
    };
    Some((action, minutes))
}

fn sync_tray_controls(
    app: &tauri::AppHandle,
    schedule: Option<&ScheduleInfo>,
) -> Result<(), String> {
    let has_schedule = schedule.is_some();
    let controls = app.state::<TrayControls>();
    controls
        .status
        .set_text(tray_status_label(schedule))
        .map_err(|error| error.to_string())?;
    controls
        .plus
        .set_enabled(has_schedule)
        .map_err(|error| error.to_string())?;
    controls
        .minus
        .set_enabled(has_schedule)
        .map_err(|error| error.to_string())?;
    controls
        .cancel
        .set_enabled(has_schedule)
        .map_err(|error| error.to_string())?;
    controls
        .keep
        .set_enabled(has_schedule)
        .map_err(|error| error.to_string())
}

fn replace_scheduled_task<Install, Persist, Remove, Rollback>(
    existing_id: Option<&str>,
    new_id: &str,
    install_new: Install,
    persist_new: Persist,
    mut remove_task: Remove,
    rollback_persistence: Rollback,
) -> Result<(), String>
where
    Install: FnOnce() -> Result<(), String>,
    Persist: FnOnce() -> Result<(), String>,
    Remove: FnMut(&str) -> Result<(), String>,
    Rollback: FnOnce() -> Result<(), String>,
{
    if existing_id == Some(new_id) {
        return Err("新计划与现有计划标识重复".to_string());
    }

    install_new()?;

    if let Err(error) = persist_new() {
        return match remove_task(new_id) {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(format!("{error}；同时清理新计划失败：{cleanup_error}")),
        };
    }

    if let Some(existing_id) = existing_id.filter(|existing_id| *existing_id != new_id) {
        if let Err(error) = remove_task(existing_id) {
            return match rollback_persistence() {
                Ok(()) => match remove_task(new_id) {
                    Ok(()) => Err(error),
                    Err(cleanup_error) => {
                        Err(format!("{error}；同时清理新计划失败：{cleanup_error}"))
                    }
                },
                Err(rollback_error) => match remove_task(existing_id) {
                    Ok(()) => Ok(()),
                    Err(retry_error) => Err(format!(
                        "{error}；恢复原计划状态失败：{rollback_error}；再次删除原计划失败：{retry_error}"
                    )),
                },
            };
        }
    }

    Ok(())
}

fn cancel_scheduled_task<Remove, Persist, Reinstall>(
    remove_task: Remove,
    persist_cancellation: Persist,
    reinstall_task: Reinstall,
) -> Result<(), String>
where
    Remove: FnOnce() -> Result<(), String>,
    Persist: FnOnce() -> Result<(), String>,
    Reinstall: FnOnce() -> Result<(), String>,
{
    remove_task()?;
    if let Err(error) = persist_cancellation() {
        return match reinstall_task() {
            Ok(()) => Err(error),
            Err(reinstall_error) => Err(format!(
                "{error}；同时恢复原计划任务失败：{reinstall_error}"
            )),
        };
    }
    Ok(())
}

fn create_schedule_now(
    app: tauri::AppHandle,
    request: ScheduleRequest,
) -> Result<ScheduleInfo, String> {
    let schedule = ScheduleInfo::from_request(request)?;
    let existing_schedule = app
        .state::<AppState>()
        .0
        .lock()
        .map_err(|_| "计划状态锁定失败".to_string())?
        .clone();
    let previous_data = storage::load(&app)?;
    let mut next_data = previous_data.clone();
    next_data.active_schedule = Some(schedule.clone());
    next_data.history.insert(
        0,
        HistoryRecord {
            time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            action: schedule.action.worker_flag().into(),
            detail: history_detail("已设定", &schedule.target_time_local),
        },
    );
    next_data.history.truncate(100);
    replace_scheduled_task(
        existing_schedule
            .as_ref()
            .map(|existing| existing.id.as_str()),
        &schedule.id,
        || windows_task::install(&schedule),
        || storage::save(&app, &next_data),
        windows_task::remove,
        || storage::save(&app, &previous_data),
    )?;
    *app.state::<AppState>()
        .0
        .lock()
        .map_err(|_| "计划状态锁定失败".to_string())? = Some(schedule.clone());
    sync_tray_controls(&app, Some(&schedule))?;
    Ok(schedule)
}

fn cancel_schedule_now(app: tauri::AppHandle) -> Result<(), String> {
    let schedule = app
        .state::<AppState>()
        .0
        .lock()
        .map_err(|_| "计划状态锁定失败".to_string())?
        .clone()
        .ok_or_else(|| "当前没有计划".to_string())?;
    let previous_data = storage::load(&app)?;
    let mut next_data = previous_data.clone();
    next_data.active_schedule = None;
    next_data.history.insert(
        0,
        HistoryRecord {
            time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            action: schedule.action.worker_flag().into(),
            detail: history_detail("已取消", &schedule.target_time_local),
        },
    );
    next_data.history.truncate(100);
    cancel_scheduled_task(
        || windows_task::remove(&schedule.id),
        || storage::save(&app, &next_data),
        || windows_task::install(&schedule),
    )?;
    *app.state::<AppState>()
        .0
        .lock()
        .map_err(|_| "计划状态锁定失败".to_string())? = None;
    sync_tray_controls(&app, None)
}

fn cancel_if_present<F>(has_schedule: bool, cancel: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    if has_schedule {
        cancel()?;
    }
    Ok(())
}

fn cancel_schedule_if_present(app: tauri::AppHandle) -> Result<(), String> {
    let has_schedule = app
        .state::<AppState>()
        .0
        .lock()
        .map_err(|_| "计划状态锁定失败".to_string())?
        .is_some();
    cancel_if_present(has_schedule, || cancel_schedule_now(app))
}

fn adjust_schedule_now(app: tauri::AppHandle, minutes: i64) -> Result<ScheduleInfo, String> {
    let current = app
        .state::<AppState>()
        .0
        .lock()
        .map_err(|_| "计划状态锁定失败".to_string())?
        .clone()
        .ok_or_else(|| "当前没有计划".to_string())?;
    let target = current.target_time + chrono::Duration::minutes(minutes);
    let request = ScheduleRequest {
        action: current.action,
        target_time: target.to_rfc3339(),
    };
    create_schedule_now(app, request)
}

#[tauri::command]
async fn create_schedule(
    app: tauri::AppHandle,
    request: ScheduleRequest,
) -> Result<ScheduleInfo, String> {
    tauri::async_runtime::spawn_blocking(move || create_schedule_now(app, request))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn cancel_schedule(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || cancel_schedule_now(app))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn adjust_schedule(app: tauri::AppHandle, minutes: i64) -> Result<ScheduleInfo, String> {
    tauri::async_runtime::spawn_blocking(move || adjust_schedule_now(app, minutes))
        .await
        .map_err(|error| error.to_string())?
}

fn run_worker_steps<Execute, MarkSuccess, MarkFailure, Remove>(
    execute: Execute,
    mark_success: MarkSuccess,
    mark_failure: MarkFailure,
    remove_task: Remove,
) -> Result<(), String>
where
    Execute: FnOnce() -> Result<(), String>,
    MarkSuccess: FnOnce() -> Result<(), String>,
    MarkFailure: FnOnce(&str) -> Result<(), String>,
    Remove: FnOnce() -> Result<(), String>,
{
    let action_result = execute();
    let history_result = match &action_result {
        Ok(()) => mark_success(),
        Err(error) => mark_failure(error),
    };
    let removal_result = remove_task();

    if let Err(mut error) = action_result {
        if let Err(history_error) = history_result {
            error.push_str(&format!("；同时记录失败结果出错：{history_error}"));
        }
        if let Err(removal_error) = removal_result {
            error.push_str(&format!("；同时清理计划任务出错：{removal_error}"));
        }
        return Err(error);
    }

    history_result?;
    removal_result
}

fn run_worker(id: &str, action_name: &str) -> Result<(), String> {
    let action = match action_name {
        "shutdown" => models::Action::Shutdown,
        "force" => models::Action::Force,
        "sleep" => models::Action::Sleep,
        _ => return Err("unknown action".into()),
    };
    run_worker_steps(
        || action::execute(action),
        || storage::mark_executed(id),
        |error| storage::mark_failed(id, error),
        || windows_task::remove(id),
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--worker") {
        let result = args
            .get(2)
            .zip(args.get(3))
            .ok_or_else(|| "worker arguments are incomplete".to_string())
            .and_then(|(id, action)| run_worker(id, action));
        if let Err(error) = result {
            eprintln!("NightOwl worker: {error}");
            std::process::exit(1);
        }
        return;
    }
    let launched_from_autostart = args.iter().any(|arg| arg == "--autostart");

    tauri::Builder::default()
        .manage(AppState(Mutex::new(None)))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .arg("--autostart")
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            use tauri::menu::{Menu, MenuItem, Submenu};
            use tauri::tray::TrayIconBuilder;

            let data = storage::load(app.handle())?;
            let silent_launch = launched_from_autostart && data.silent_startup;
            let active_schedule = data.active_schedule;
            let status = MenuItem::with_id(
                app,
                "status",
                tray_status_label(active_schedule.as_ref()),
                false,
                None::<&str>,
            )?;
            *app.state::<AppState>()
                .0
                .lock()
                .map_err(|_| "计划状态锁定失败")? = active_schedule.clone();
            let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let plus = MenuItem::with_id(
                app,
                "plus",
                "增加 30 分钟",
                active_schedule.is_some(),
                None::<&str>,
            )?;
            let minus = MenuItem::with_id(
                app,
                "minus",
                "减少 30 分钟",
                active_schedule.is_some(),
                None::<&str>,
            )?;
            let cancel = MenuItem::with_id(
                app,
                "cancel",
                "取消计划",
                active_schedule.is_some(),
                None::<&str>,
            )?;
            let settings = MenuItem::with_id(app, "settings", "打开设置", true, None::<&str>)?;
            let quick_shutdown_30 =
                MenuItem::with_id(app, "quick-shutdown-30", "30 分钟", true, None::<&str>)?;
            let quick_shutdown_60 =
                MenuItem::with_id(app, "quick-shutdown-60", "60 分钟", true, None::<&str>)?;
            let quick_shutdown_90 =
                MenuItem::with_id(app, "quick-shutdown-90", "90 分钟", true, None::<&str>)?;
            let quick_shutdown_120 =
                MenuItem::with_id(app, "quick-shutdown-120", "120 分钟", true, None::<&str>)?;
            let quick_sleep_30 =
                MenuItem::with_id(app, "quick-sleep-30", "30 分钟", true, None::<&str>)?;
            let quick_sleep_60 =
                MenuItem::with_id(app, "quick-sleep-60", "60 分钟", true, None::<&str>)?;
            let quick_sleep_90 =
                MenuItem::with_id(app, "quick-sleep-90", "90 分钟", true, None::<&str>)?;
            let quick_sleep_120 =
                MenuItem::with_id(app, "quick-sleep-120", "120 分钟", true, None::<&str>)?;
            let quick_force_30 =
                MenuItem::with_id(app, "quick-force-30", "30 分钟", true, None::<&str>)?;
            let quick_force_60 =
                MenuItem::with_id(app, "quick-force-60", "60 分钟", true, None::<&str>)?;
            let quick_force_90 =
                MenuItem::with_id(app, "quick-force-90", "90 分钟", true, None::<&str>)?;
            let quick_force_120 =
                MenuItem::with_id(app, "quick-force-120", "120 分钟", true, None::<&str>)?;
            let quick_shutdown = Submenu::with_items(
                app,
                "正常关机",
                true,
                &[
                    &quick_shutdown_30,
                    &quick_shutdown_60,
                    &quick_shutdown_90,
                    &quick_shutdown_120,
                ],
            )?;
            let quick_sleep = Submenu::with_items(
                app,
                "睡眠",
                true,
                &[
                    &quick_sleep_30,
                    &quick_sleep_60,
                    &quick_sleep_90,
                    &quick_sleep_120,
                ],
            )?;
            let quick_force = Submenu::with_items(
                app,
                "强制关机",
                true,
                &[
                    &quick_force_30,
                    &quick_force_60,
                    &quick_force_90,
                    &quick_force_120,
                ],
            )?;
            let quick_create = Submenu::with_items(
                app,
                "快速创建",
                true,
                &[&quick_shutdown, &quick_sleep, &quick_force],
            )?;
            let keep = MenuItem::with_id(app, "keep", "退出并保留计划", true, None::<&str>)?;
            let exit = MenuItem::with_id(app, "exit", "退出并取消计划", true, None::<&str>)?;
            keep.set_enabled(active_schedule.is_some())?;
            let menu = Menu::with_items(
                app,
                &[
                    &status,
                    &show,
                    &quick_create,
                    &plus,
                    &minus,
                    &cancel,
                    &settings,
                    &keep,
                    &exit,
                ],
            )?;
            app.manage(TrayControls {
                status,
                plus,
                minus,
                cancel,
                keep,
            });
            TrayIconBuilder::new()
                .icon(tauri::include_image!("icons/icon.png"))
                .menu(&menu)
                .tooltip("NightOwl 定时助手")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        let _ = app.emit("tray-open-settings", ());
                    }
                    "cancel" => {
                        let _ = app.emit("tray-cancel", ());
                    }
                    "plus" => {
                        let _ = app.emit("tray-adjust", 30_i64);
                    }
                    "minus" => {
                        let _ = app.emit("tray-adjust", -30_i64);
                    }
                    "keep" => app.exit(0),
                    "exit" => {
                        if cancel_schedule_if_present(app.clone()).is_ok() {
                            app.exit(0);
                        } else if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    quick_id => {
                        if let Some((action, minutes)) = parse_quick_schedule_id(quick_id) {
                            if action == models::Action::Force {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                            let _ = app
                                .emit("tray-quick-schedule", TrayQuickSchedule { action, minutes });
                        }
                    }
                })
                .build(app)?;
            if !silent_launch {
                if let Some(window) = app.get_webview_window("main") {
                    window.show()?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_schedule,
            cancel_schedule,
            adjust_schedule,
            get_history,
            get_active_schedule,
            get_settings,
            save_settings,
            get_diagnostics,
            minimize_window,
            hide_window,
            exit_application,
            open_repository,
            open_data_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running NightOwl");
}

#[cfg(test)]
mod tests {
    use super::{
        cancel_if_present, cancel_scheduled_task, configure_autostart, history_detail,
        normalized_silent_startup, parse_quick_schedule_id, replace_scheduled_task,
        run_worker_steps, tray_status_label,
    };
    use crate::models::{Action, ScheduleInfo};
    use chrono::{DateTime, Utc};

    #[test]
    fn tray_status_describes_the_current_plan_or_its_absence() {
        assert_eq!(tray_status_label(None), "当前无计划");
        let schedule = ScheduleInfo {
            id: "test".into(),
            action: Action::Sleep,
            target_time: DateTime::parse_from_rfc3339("2030-01-01T01:30:00Z")
                .expect("test datetime should parse")
                .with_timezone(&Utc),
            target_time_local: "2030-01-01 09:30:00".into(),
        };

        assert_eq!(
            tray_status_label(Some(&schedule)),
            "睡眠 · 2030-01-01 09:30"
        );
    }

    #[test]
    fn history_detail_distinguishes_scheduled_cancelled_and_executed() {
        assert_eq!(
            history_detail("已设定", "2030-01-01 09:30:00"),
            "已设定 · 2030-01-01 09:30:00"
        );
        assert_eq!(
            history_detail("已取消", "2030-01-01 09:30:00"),
            "已取消 · 2030-01-01 09:30:00"
        );
        assert_eq!(
            history_detail("已执行", "2030-01-01 09:30:00"),
            "已执行 · 2030-01-01 09:30:00"
        );
    }

    #[test]
    fn parses_each_tray_quick_schedule_action_and_duration() {
        assert_eq!(
            parse_quick_schedule_id("quick-shutdown-30"),
            Some((Action::Shutdown, 30))
        );
        assert_eq!(
            parse_quick_schedule_id("quick-sleep-60"),
            Some((Action::Sleep, 60))
        );
        assert_eq!(
            parse_quick_schedule_id("quick-force-120"),
            Some((Action::Force, 120))
        );
        assert_eq!(parse_quick_schedule_id("quick-force-45"), None);
        assert_eq!(parse_quick_schedule_id("quick-30"), None);
    }

    #[test]
    fn exit_cancellation_runs_before_exit_when_a_schedule_exists() {
        let mut cancelled = false;

        cancel_if_present(true, || {
            cancelled = true;
            Ok(())
        })
        .expect("active schedule should be cancelled");

        assert!(cancelled);
    }

    #[test]
    fn exit_cancellation_skips_cancel_when_no_schedule_exists() {
        cancel_if_present(false, || -> Result<(), String> {
            panic!("cancel should not run without an active schedule")
        })
        .expect("exiting without a schedule should succeed");
    }

    #[test]
    fn exit_cancellation_propagates_failure_and_prevents_exit() {
        let result = cancel_if_present(true, || Err("cancel failed".to_string()));

        assert_eq!(result, Err("cancel failed".to_string()));
    }

    #[test]
    fn schedule_replacement_installs_persists_then_removes_old_task_once() {
        let operations = std::cell::RefCell::new(Vec::new());

        replace_scheduled_task(
            Some("old"),
            "new",
            || {
                operations.borrow_mut().push("install-new".to_string());
                Ok(())
            },
            || {
                operations.borrow_mut().push("persist-new".to_string());
                Ok(())
            },
            |id| {
                operations.borrow_mut().push(format!("remove-{id}"));
                Ok(())
            },
            || {
                operations.borrow_mut().push("rollback".to_string());
                Ok(())
            },
        )
        .expect("replacement should succeed");

        assert_eq!(
            operations.into_inner(),
            ["install-new", "persist-new", "remove-old"]
        );
    }

    #[test]
    fn schedule_replacement_removes_new_task_when_persistence_fails() {
        let removed = std::cell::RefCell::new(Vec::new());

        let result = replace_scheduled_task(
            Some("old"),
            "new",
            || Ok(()),
            || Err("save failed".to_string()),
            |id| {
                removed.borrow_mut().push(id.to_string());
                Ok(())
            },
            || Ok(()),
        );

        assert_eq!(result, Err("save failed".to_string()));
        assert_eq!(removed.into_inner(), ["new"]);
    }

    #[test]
    fn schedule_replacement_rejects_the_same_id_before_installing() {
        let operations = std::cell::RefCell::new(Vec::new());

        let result = replace_scheduled_task(
            Some("same"),
            "same",
            || {
                operations.borrow_mut().push("install".to_string());
                Ok(())
            },
            || {
                operations.borrow_mut().push("persist".to_string());
                Ok(())
            },
            |id| {
                operations.borrow_mut().push(id.to_string());
                Ok(())
            },
            || {
                operations.borrow_mut().push("rollback".to_string());
                Ok(())
            },
        );

        assert_eq!(result, Err("新计划与现有计划标识重复".to_string()));
        assert!(operations.into_inner().is_empty());
    }

    #[test]
    fn schedule_replacement_rolls_back_when_old_task_removal_fails() {
        let operations = std::cell::RefCell::new(Vec::new());

        let result = replace_scheduled_task(
            Some("old"),
            "new",
            || {
                operations.borrow_mut().push("install-new".to_string());
                Ok(())
            },
            || {
                operations.borrow_mut().push("persist-new".to_string());
                Ok(())
            },
            |id| {
                operations.borrow_mut().push(format!("remove-{id}"));
                if id == "old" {
                    Err("remove old failed".to_string())
                } else {
                    Ok(())
                }
            },
            || {
                operations.borrow_mut().push("rollback".to_string());
                Ok(())
            },
        );

        assert_eq!(result, Err("remove old failed".to_string()));
        assert_eq!(
            operations.into_inner(),
            [
                "install-new",
                "persist-new",
                "remove-old",
                "rollback",
                "remove-new"
            ]
        );
    }

    #[test]
    fn schedule_replacement_keeps_new_state_when_rollback_fails_but_retry_succeeds() {
        let remove_attempts = std::cell::Cell::new(0);

        let result = replace_scheduled_task(
            Some("old"),
            "new",
            || Ok(()),
            || Ok(()),
            |id| {
                if id == "old" {
                    remove_attempts.set(remove_attempts.get() + 1);
                    if remove_attempts.get() == 1 {
                        return Err("remove old failed".to_string());
                    }
                }
                Ok(())
            },
            || Err("rollback failed".to_string()),
        );

        assert_eq!(result, Ok(()));
        assert_eq!(remove_attempts.get(), 2);
    }

    #[test]
    fn cancellation_reinstalls_task_when_persistence_fails() {
        let operations = std::cell::RefCell::new(Vec::new());

        let result = cancel_scheduled_task(
            || {
                operations.borrow_mut().push("remove");
                Ok(())
            },
            || {
                operations.borrow_mut().push("persist");
                Err("save failed".to_string())
            },
            || {
                operations.borrow_mut().push("reinstall");
                Ok(())
            },
        );

        assert_eq!(result, Err("save failed".to_string()));
        assert_eq!(operations.into_inner(), ["remove", "persist", "reinstall"]);
    }

    #[test]
    fn silent_startup_requires_autostart() {
        assert!(!normalized_silent_startup(false, true));
        assert!(normalized_silent_startup(true, true));
        assert!(!normalized_silent_startup(true, false));
    }

    #[test]
    fn autostart_configuration_uses_the_matching_operation() {
        let operations = std::cell::RefCell::new(Vec::new());

        configure_autostart(
            true,
            || {
                operations.borrow_mut().push("enable");
                Ok(())
            },
            || {
                operations.borrow_mut().push("disable");
                Ok(())
            },
        )
        .expect("autostart should enable");
        configure_autostart(
            false,
            || {
                operations.borrow_mut().push("enable");
                Ok(())
            },
            || {
                operations.borrow_mut().push("disable");
                Ok(())
            },
        )
        .expect("autostart should disable");

        assert_eq!(operations.into_inner(), ["enable", "disable"]);
    }

    #[test]
    fn worker_records_success_only_after_the_action_succeeds() {
        let operations = std::cell::RefCell::new(Vec::new());

        run_worker_steps(
            || {
                operations.borrow_mut().push("execute".to_string());
                Ok(())
            },
            || {
                operations.borrow_mut().push("success".to_string());
                Ok(())
            },
            |_| {
                operations.borrow_mut().push("failure".to_string());
                Ok(())
            },
            || {
                operations.borrow_mut().push("remove".to_string());
                Ok(())
            },
        )
        .expect("successful worker should complete");

        assert_eq!(operations.into_inner(), ["execute", "success", "remove"]);
    }

    #[test]
    fn worker_records_failure_when_the_action_fails() {
        let operations = std::cell::RefCell::new(Vec::new());

        let result = run_worker_steps(
            || {
                operations.borrow_mut().push("execute".to_string());
                Err("action failed".to_string())
            },
            || {
                operations.borrow_mut().push("success".to_string());
                Ok(())
            },
            |error| {
                operations.borrow_mut().push(format!("failure-{error}"));
                Ok(())
            },
            || {
                operations.borrow_mut().push("remove".to_string());
                Ok(())
            },
        );

        assert_eq!(result, Err("action failed".to_string()));
        assert_eq!(
            operations.into_inner(),
            ["execute", "failure-action failed", "remove"]
        );
    }
}
