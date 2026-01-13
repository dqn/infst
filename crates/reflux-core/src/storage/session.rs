use crate::error::Result;
use chrono::{DateTime, Local};
use std::fs::{self};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct SessionManager {
    base_dir: PathBuf,
    current_session: Option<PathBuf>,
}

impl SessionManager {
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            current_session: None,
        }
    }

    pub fn start_session(&mut self) -> Result<PathBuf> {
        let now: DateTime<Local> = Local::now();
        let session_dir = self.base_dir.join(now.format("%Y-%m-%d").to_string());
        fs::create_dir_all(&session_dir)?;

        let session_file = session_dir.join(format!("session_{}.tsv", now.format("%H%M%S")));
        self.current_session = Some(session_file.clone());

        Ok(session_file)
    }

    pub fn append_line(&self, line: &str) -> Result<()> {
        if let Some(ref path) = self.current_session {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }

    pub fn current_session_path(&self) -> Option<&Path> {
        self.current_session.as_deref()
    }
}
