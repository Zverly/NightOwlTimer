use crate::models::Action;
use std::process::Command;

pub fn execute(action: Action) -> Result<(), String> {
    let mut command = Command::new("shutdown.exe");
    match action {
        Action::Shutdown => {
            command.args(["/s", "/t", "0"]);
        }
        Action::Force => {
            command.args(["/s", "/f", "/t", "0"]);
        }
        Action::Sleep => {
            return Command::new("rundll32.exe")
                .args(["powrprof.dll,SetSuspendState", "0,1,0"])
                .status()
                .map_err(|e| e.to_string())
                .and_then(|s| {
                    if s.success() {
                        Ok(())
                    } else {
                        Err(format!("睡眠命令退出码 {:?}", s.code()))
                    }
                });
        }
    }
    command.status().map_err(|e| e.to_string()).and_then(|s| {
        if s.success() {
            Ok(())
        } else {
            Err(format!("关机命令退出码 {:?}", s.code()))
        }
    })
}
