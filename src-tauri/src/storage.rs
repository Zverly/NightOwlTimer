use crate::models::{AppData, HistoryRecord};
use std::{fs, path::Path, path::PathBuf};

const APP_DATA_DIRECTORY_NAME: &str = "NightOwl Timer";
const LEGACY_DATA_FILE: &str = r"E:\NightOwl\data\nightowl-data.json";

fn resolve_data_directory(app_data: Option<PathBuf>, executable: Option<PathBuf>) -> PathBuf {
    if let Some(app_data) = app_data {
        return app_data.join(APP_DATA_DIRECTORY_NAME);
    }

    executable
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("data")
}

fn migrate_data_file(legacy_file: &Path, current_file: &Path) -> Result<(), String> {
    if current_file.exists() || !legacy_file.exists() {
        return Ok(());
    }

    if let Some(parent) = current_file.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::copy(legacy_file, current_file)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn data_file() -> Result<PathBuf, String> {
    let directory = data_directory();
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let file = data_file_path();
    migrate_data_file(Path::new(LEGACY_DATA_FILE), &file)?;
    Ok(file)
}

pub fn data_directory() -> PathBuf {
    resolve_data_directory(None, std::env::current_exe().ok())
}

pub fn data_file_path() -> PathBuf {
    data_directory().join("nightowl-data.json")
}

fn load_data() -> Result<AppData, String> {
    let file = data_file()?;
    if !file.exists() {
        let data = AppData::default();
        let text = serde_json::to_string_pretty(&data).map_err(|error| error.to_string())?;
        fs::write(&file, text).map_err(|error| error.to_string())?;
        return Ok(data);
    }
    let text = fs::read_to_string(file).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

fn save_data(data: &AppData) -> Result<(), String> {
    let text = serde_json::to_string_pretty(data).map_err(|error| error.to_string())?;
    fs::write(data_file()?, text).map_err(|error| error.to_string())
}

pub fn load(_app: &tauri::AppHandle) -> Result<AppData, String> {
    load_data()
}

pub fn save(_app: &tauri::AppHandle, data: &AppData) -> Result<(), String> {
    save_data(data)
}

fn mark_finished(id: &str, status: &str) -> Result<(), String> {
    let mut data = load_data()?;
    let Some(schedule) = data.active_schedule.take() else {
        return Ok(());
    };
    if schedule.id != id {
        data.active_schedule = Some(schedule);
        return save_data(&data);
    }
    data.history.insert(
        0,
        HistoryRecord {
            time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            action: schedule.action.worker_flag().into(),
            detail: format!("{status} · {}", schedule.target_time_local),
        },
    );
    data.history.truncate(100);
    save_data(&data)
}

pub fn mark_executed(id: &str) -> Result<(), String> {
    mark_finished(id, "已执行")
}

pub fn mark_failed(id: &str, error: &str) -> Result<(), String> {
    mark_finished(id, &format!("执行失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::{migrate_data_file, resolve_data_directory};
    use std::{fs, path::PathBuf};

    #[test]
    fn data_directory_uses_the_executable_directory() {
        let directory = resolve_data_directory(
            None,
            Some(PathBuf::from(r"D:\Apps\NightOwl\nightowl-timer.exe")),
        );

        assert_eq!(directory, PathBuf::from(r"D:\Apps\NightOwl\data"));
    }

    #[test]
    fn data_directory_falls_back_to_the_installed_executable() {
        let directory = resolve_data_directory(
            None,
            Some(PathBuf::from(r"D:\Apps\NightOwl\nightowl-timer.exe")),
        );

        assert_eq!(directory, PathBuf::from(r"D:\Apps\NightOwl\data"));
    }

    #[test]
    fn migration_copies_legacy_data_without_overwriting_newer_data() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "nightowl-storage-test-{}-{unique}",
            std::process::id()
        ));
        let legacy = root.join("legacy.json");
        let current = root.join("current").join("nightowl-data.json");
        fs::create_dir_all(&root).expect("test directory should be created");
        fs::write(&legacy, "legacy").expect("legacy data should be written");

        migrate_data_file(&legacy, &current).expect("legacy data should migrate");
        assert_eq!(
            fs::read_to_string(&current).expect("migrated data should be readable"),
            "legacy"
        );

        fs::write(&current, "current").expect("current data should be written");
        migrate_data_file(&legacy, &current).expect("existing data should be preserved");
        assert_eq!(
            fs::read_to_string(&current).expect("current data should be readable"),
            "current"
        );

        fs::remove_dir_all(root).expect("test directory should be removed");
    }
}
