//! The tools shipped next to the app (see `scripts/sidecars.ts`) and a way to run them with a
//! deadline. Nothing here goes through a shell: arguments are passed as they are.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone)]
pub struct Sidecars {
    dir: PathBuf,
}

impl Sidecars {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The bundled resources, in dev (no bundle yet) the folder the fetch script fills.
    pub fn locate(app: &AppHandle) -> Result<Self> {
        let resources = app.path().resource_dir().context("no resource directory")?;
        // without the `\\?\` Windows puts in front: Java crashes on a jar given that way
        let bundled = dunce::simplified(&resources).join("sidecars");
        let dir = if bundled.join("mustang.jar").exists() {
            bundled
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sidecars")
        };
        let found = Self::new(dir);
        if !found.java().exists() || !found.mustang().exists() {
            bail!(
                "sidecars missing in {}, run `bun run generate:sidecars`",
                found.dir.display()
            );
        }
        Ok(found)
    }

    /// The folder `bun run generate:sidecars` fills, for the tests that need the real tools.
    #[cfg(test)]
    pub fn in_repo() -> Self {
        Self::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sidecars"))
    }

    pub fn java(&self) -> PathBuf {
        self.dir
            .join("jre")
            .join("bin")
            .join(format!("java{}", std::env::consts::EXE_SUFFIX))
    }

    pub fn mustang(&self) -> PathBuf {
        self.dir.join("mustang.jar")
    }
}

#[derive(Debug)]
pub struct Output {
    /// `None` when the process ended without a code (signal)
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Runs a program to its end, or kills it after `timeout`.
pub fn run(program: &Path, args: &[OsString], cwd: &Path, timeout: Duration) -> Result<Output> {
    let mut command = Command::new(program);
    // the app has no console, without this every run would open one (CREATE_NO_WINDOW)
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    let mut child = command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("cannot start {}", program.display()))?;

    // drained on their own threads, a full pipe would stall the child while we wait for it
    let drain = |stream: Option<Box<dyn Read + Send>>| {
        thread::spawn(move || {
            let mut buffer = Vec::new();
            if let Some(mut stream) = stream {
                let _ = stream.read_to_end(&mut buffer);
            }
            String::from_utf8_lossy(&buffer).into_owned()
        })
    };
    let stdout = drain(child.stdout.take().map(|s| Box::new(s) as _));
    let stderr = drain(child.stderr.take().map(|s| Box::new(s) as _));

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "{} did not finish within {}s",
                program.display(),
                timeout.as_secs()
            );
        }
        thread::sleep(Duration::from_millis(25));
    };

    Ok(Output {
        code: status.code(),
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn captures_output_and_exit_code() {
        let out = run(
            Path::new("/bin/sh"),
            &["-c".into(), "echo hi; echo oops >&2; exit 3".into()],
            Path::new("/"),
            Duration::from_secs(5),
        )
        .unwrap();

        assert_eq!(out.code, Some(3));
        assert_eq!(out.stdout.trim(), "hi");
        assert_eq!(out.stderr.trim(), "oops");
    }

    #[cfg(unix)]
    #[test]
    fn kills_what_runs_too_long() {
        let result = run(
            Path::new("/bin/sleep"),
            &["5".into()],
            Path::new("/"),
            Duration::from_millis(100),
        );

        assert!(result.unwrap_err().to_string().contains("did not finish"));
    }

    #[cfg(windows)]
    #[test]
    fn captures_output_and_exit_code() {
        let out = run(
            Path::new(r"C:\Windows\System32\cmd.exe"),
            &["/C".into(), "echo hi& echo oops 1>&2& exit /b 3".into()],
            Path::new(r"C:\"),
            Duration::from_secs(5),
        )
        .unwrap();

        assert_eq!(out.code, Some(3));
        assert_eq!(out.stdout.trim(), "hi");
        assert_eq!(out.stderr.trim(), "oops");
    }

    #[cfg(windows)]
    #[test]
    fn kills_what_runs_too_long() {
        let result = run(
            Path::new(r"C:\Windows\System32\PING.EXE"),
            &["-n".into(), "6".into(), "127.0.0.1".into()],
            Path::new(r"C:\"),
            Duration::from_millis(100),
        );

        assert!(result.unwrap_err().to_string().contains("did not finish"));
    }
}
