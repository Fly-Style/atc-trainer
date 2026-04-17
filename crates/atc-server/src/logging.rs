use atc_shared::ids::{AircraftId, SessionId};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Per-session log sink. Writes one human-readable line per accepted action.
pub struct SessionLog {
    session_id: SessionId,
    file: Option<Mutex<std::fs::File>>,
}

impl SessionLog {
    pub fn new(session_id: SessionId, log_dir: Option<&std::path::Path>) -> std::io::Result<Self> {
        let file = if let Some(dir) = log_dir {
            std::fs::create_dir_all(dir)?;
            let path: PathBuf = dir.join(format!("{}.log", session_id.as_str()));
            let f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
            Some(Mutex::new(f))
        } else {
            None
        };
        Ok(Self { session_id, file })
    }

    pub fn write(
        &self,
        actor: &str,
        aircraft: Option<&AircraftId>,
        action: &str,
        result: &str,
    ) {
        let ts = OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default();
        let aircraft_part = aircraft
            .map(|a| format!(" aircraft={}", a.as_str()))
            .unwrap_or_default();
        let line = format!(
            "{ts} session={sid} actor={actor}{aircraft_part} action={action} result=\"{result}\"\n",
            sid = self.session_id.as_str(),
        );
        if let Some(lock) = &self.file {
            if let Ok(mut f) = lock.lock() {
                let _ = f.write_all(line.as_bytes());
            }
        }
        tracing::info!(target: "session_log", "{}", line.trim_end());
    }
}
