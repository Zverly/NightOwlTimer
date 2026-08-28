use crate::models::ScheduleInfo;
use std::os::windows::process::CommandExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const TASK_PREFIX: &str = "NightOwlTimer-";

fn run_schtasks(args: &[&str]) -> Result<(), String> {
    let output = Command::new("schtasks.exe")
        .creation_flags(0x08000000)
        .args(args)
        .output()
        .map_err(|e| format!("无法启动 Windows 计划任务: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = format!("{stderr}\n{stdout}").trim().to_string();
        Err(if detail.is_empty() {
            "Windows Task Scheduler failed".into()
        } else {
            detail
        })
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn task_xml(schedule: &ScheduleInfo, executable: &Path) -> String {
    let boundary = schedule
        .target_time
        .with_timezone(&chrono::Local)
        .to_rfc3339();
    let command = escape_xml(&executable.display().to_string());
    let arguments = format!("--worker {} {}", schedule.id, schedule.action.worker_flag());
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Author>NightOwl</Author></RegistrationInfo>
  <Triggers><TimeTrigger><StartBoundary>{boundary}</StartBoundary><Enabled>true</Enabled></TimeTrigger></Triggers>
  <Principals><Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><AllowHardTerminate>true</AllowHardTerminate><StartWhenAvailable>true</StartWhenAvailable><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><Priority>7</Priority></Settings>
  <Actions Context="Author"><Exec><Command>{command}</Command><Arguments>{}</Arguments></Exec></Actions>
</Task>"#,
        escape_xml(&arguments)
    )
}

fn write_task_xml(path: &Path, content: &str) -> Result<(), std::io::Error> {
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(content.encode_utf16().flat_map(u16::to_le_bytes));
    fs::write(path, bytes)
}

fn task_xml_path(id: &str) -> PathBuf {
    crate::storage::data_directory().join(format!("{TASK_PREFIX}{id}.xml"))
}

pub fn remove(id: &str) -> Result<(), String> {
    run_schtasks(&["/Delete", "/TN", &format!("{TASK_PREFIX}{id}"), "/F"])
}

pub fn install(schedule: &ScheduleInfo) -> Result<(), String> {
    let _ = remove(&schedule.id);
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let task_name = format!("{TASK_PREFIX}{}", schedule.id);
    let xml_path = task_xml_path(&schedule.id);
    let parent = xml_path.parent().expect("task XML path has a parent");
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    write_task_xml(&xml_path, &task_xml(schedule, &exe)).map_err(|error| error.to_string())?;
    let result = run_schtasks(&[
        "/Create",
        "/TN",
        &task_name,
        "/XML",
        xml_path.to_string_lossy().as_ref(),
        "/F",
    ]);
    let _ = fs::remove_file(xml_path);
    result
}

#[cfg(test)]
mod tests {
    use super::task_xml;
    use crate::models::{Action, ScheduleInfo};
    use chrono::DateTime;
    use std::path::Path;

    #[test]
    fn task_xml_uses_an_iso_time_trigger_and_separate_arguments() {
        let schedule = ScheduleInfo {
            id: "nightowl-123".into(),
            action: Action::Sleep,
            target_time: DateTime::parse_from_rfc3339("2026-08-29T15:05:00+08:00")
                .expect("test datetime should parse")
                .to_utc(),
            target_time_local: "2026-08-29 15:05:00".into(),
        };

        let xml = task_xml(&schedule, Path::new("E:/NightOwl/nightowl-timer.exe"));

        assert!(xml.contains("<StartBoundary>2026-08-29T15:05:00+08:00</StartBoundary>"));
        assert!(xml.contains("<Command>E:/NightOwl/nightowl-timer.exe</Command>"));
        assert!(xml.contains("<Arguments>--worker nightowl-123 sleep</Arguments>"));
    }

    #[test]
    #[ignore = "uses the Windows Task Scheduler"]
    fn installs_and_removes_a_windows_task() {
        let schedule = ScheduleInfo {
            id: format!("integration-{}", chrono::Utc::now().timestamp()),
            action: Action::Sleep,
            target_time: chrono::Utc::now() + chrono::Duration::minutes(5),
            target_time_local: "integration test".into(),
        };

        super::install(&schedule).expect("task should install");
        let task_name = format!("\\NightOwlTimer-{}", schedule.id);
        let query = std::process::Command::new("schtasks.exe")
            .args(["/Query", "/TN", &task_name])
            .output()
            .expect("task query should run");
        let removal = super::remove(&schedule.id);

        assert!(
            query.status.success(),
            "task should be discoverable: {}",
            String::from_utf8_lossy(&query.stderr)
        );
        removal.expect("task should be removable");
    }
}
