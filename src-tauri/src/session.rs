//! Which work directories hold files that passed the validation gate.
//!
//! The gate is enforced here, on the Rust side: the UI cannot export a file that was never
//! validated or that failed, whatever it sends.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::model::Settings;

#[derive(Debug, Clone)]
pub struct Session {
    pub valid: bool,
    pub pdf: PathBuf,
    pub xml: PathBuf,
    /// The stored invoice the files were made from
    pub invoice_id: i64,
    /// The number printed on them, final only once the invoice is exported
    pub number: String,
    /// The settings in force when they were made, frozen into the invoice on export
    pub settings: Settings,
    pub total: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Artifact {
    Pdf,
    Xml,
}

#[derive(Default)]
pub struct Sessions(Mutex<HashMap<PathBuf, Session>>);

impl Sessions {
    pub fn insert(&self, dir: PathBuf, session: Session) {
        self.0.lock().expect("sessions lock").insert(dir, session);
    }

    /// The session of a known work directory, only if its report was valid and it belongs to the
    /// invoice: the gate that stands between produced files and the user's disk.
    pub fn exportable(&self, dir: &Path, invoice_id: i64) -> Result<Session, String> {
        let sessions = self.0.lock().expect("sessions lock");
        let session = sessions.get(dir).ok_or("unknown work directory")?;
        if session.invoice_id != invoice_id {
            return Err("the files belong to another invoice".into());
        }
        if !session.valid {
            return Err("the invoice did not pass validation, it cannot be saved".into());
        }
        Ok(session.clone())
    }

    pub fn file(session: &Session, artifact: Artifact) -> PathBuf {
        match artifact {
            Artifact::Pdf => session.pdf.clone(),
            Artifact::Xml => session.xml.clone(),
        }
    }

    /// Forgets a work directory and deletes it, unknown paths are left alone.
    pub fn discard(&self, dir: &Path) {
        if self.0.lock().expect("sessions lock").remove(dir).is_some() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    pub fn discard_all(&self) {
        let dirs: Vec<_> = self
            .0
            .lock()
            .expect("sessions lock")
            .keys()
            .cloned()
            .collect();
        dirs.iter().for_each(|dir| self.discard(dir));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(valid: bool) -> Session {
        Session {
            valid,
            pdf: "/w/a.pdf".into(),
            xml: "/w/a.xml".into(),
            invoice_id: 7,
            number: "RE-2026-001".into(),
            settings: Settings::default(),
            total: "1.00".into(),
        }
    }

    #[test]
    fn only_valid_sessions_of_the_right_invoice_export() {
        let sessions = Sessions::default();
        sessions.insert("/w/ok".into(), session(true));
        sessions.insert("/w/bad".into(), session(false));

        let ok = sessions.exportable(Path::new("/w/ok"), 7).unwrap();
        assert_eq!(
            Sessions::file(&ok, Artifact::Pdf),
            PathBuf::from("/w/a.pdf")
        );
        assert_eq!(
            Sessions::file(&ok, Artifact::Xml),
            PathBuf::from("/w/a.xml")
        );
        assert!(sessions.exportable(Path::new("/w/ok"), 8).is_err());
        assert!(sessions.exportable(Path::new("/w/bad"), 7).is_err());
        assert!(sessions.exportable(Path::new("/elsewhere"), 7).is_err());
    }

    #[test]
    fn discard_deletes_known_directories_only() {
        let known = tempfile::tempdir().unwrap().keep();
        let stranger = tempfile::tempdir().unwrap();
        let sessions = Sessions::default();
        sessions.insert(known.clone(), session(true));

        sessions.discard(&known);
        sessions.discard(stranger.path());

        assert!(!known.exists());
        assert!(stranger.path().exists());
    }
}
