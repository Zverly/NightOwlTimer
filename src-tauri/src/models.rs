use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleRequest {
    pub action: Action,
    pub target_time: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Shutdown,
    Force,
    Sleep,
}

impl Action {
    pub fn worker_flag(&self) -> &'static str {
        match self {
            Self::Shutdown => "shutdown",
            Self::Force => "force",
            Self::Sleep => "sleep",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScheduleInfo {
    pub id: String,
    pub action: Action,
    pub target_time: DateTime<Utc>,
    pub target_time_local: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HistoryRecord {
    pub time: String,
    pub action: String,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppData {
    pub history: Vec<HistoryRecord>,
    pub active_schedule: Option<ScheduleInfo>,
    pub reminders: ReminderSettings,
    pub launch_at_startup: bool,
    pub silent_startup: bool,
    #[serde(default = "default_exit_to_tray")]
    pub exit_to_tray: bool,
}

fn default_exit_to_tray() -> bool {
    true
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            active_schedule: None,
            reminders: ReminderSettings::default(),
            launch_at_startup: false,
            silent_startup: false,
            exit_to_tray: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReminderSettings {
    pub ten_minutes: bool,
    pub one_minute: bool,
    pub thirty_seconds: bool,
}

impl Default for ReminderSettings {
    fn default() -> Self {
        Self {
            ten_minutes: true,
            one_minute: true,
            thirty_seconds: true,
        }
    }
}

impl ScheduleInfo {
    pub fn from_request(request: ScheduleRequest) -> Result<Self, String> {
        let target_time = DateTime::parse_from_rfc3339(&request.target_time)
            .map_err(|_| "无法解析计划时间".to_string())?
            .with_timezone(&Utc);
        if target_time <= Utc::now() + chrono::Duration::seconds(30) {
            return Err("计划时间必须晚于当前时间 30 秒".into());
        }
        Ok(Self {
            id: format!("nightowl-{}", target_time.timestamp()),
            action: request.action,
            target_time,
            target_time_local: target_time
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::AppData;

    #[test]
    fn loads_a_persisted_active_schedule() {
        let data: AppData = serde_json::from_str(
            r#"{
          "history": [],
          "reminders": {"ten_minutes": true, "one_minute": true, "thirty_seconds": true},
          "launch_at_startup": false,
          "silent_startup": false,
          "active_schedule": {
            "id": "nightowl-1893456000",
            "action": "shutdown",
            "target_time": "2030-01-01T00:00:00Z",
            "target_time_local": "2030-01-01 08:00:00"
          }
        }"#,
        )
        .expect("saved data should deserialize");

        assert_eq!(
            data.active_schedule
                .expect("active schedule should be restored")
                .id,
            "nightowl-1893456000"
        );
    }
}
