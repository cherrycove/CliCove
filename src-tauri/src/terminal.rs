use crate::models::{TerminalExit, TerminalOutput, TerminalSession};
use crate::storage::Storage;
use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

struct TerminalHandle {
    session: Arc<Mutex<TerminalSession>>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    ended: Arc<AtomicBool>,
}

pub struct TerminalManager {
    spawn_gate: Mutex<()>,
    sessions: Mutex<HashMap<String, Arc<TerminalHandle>>>,
    storage: Arc<Storage>,
}

impl TerminalManager {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            spawn_gate: Mutex::new(()),
            sessions: Mutex::new(HashMap::new()),
            storage,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &self,
        session_id: Option<String>,
        app: AppHandle,
        tool_id: String,
        title: String,
        executable: String,
        args: Vec<String>,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        login_path: OsString,
        kind: String,
    ) -> Result<TerminalSession, String> {
        let _gate = self.spawn_gate.lock();
        if !Path::new(&executable).is_file() {
            return Err(format!("可执行文件不存在：{executable}"));
        }
        if !cwd.is_dir() {
            return Err(format!("工作目录不存在：{}", cwd.display()));
        }

        let pair = native_pty_system()
            .openpty(PtySize {
                rows: rows.clamp(5, 300),
                cols: cols.clamp(20, 500),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| error.to_string())?;

        let mut command = pty_command(&executable, &args);
        command.cwd(&cwd);
        configure_terminal_environment(&mut command, &login_path);

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| error.to_string())?;
        drop(pair.slave);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| error.to_string())?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| error.to_string())?;
        let session_id = session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let session = TerminalSession {
            id: session_id.clone(),
            tool_id,
            title,
            executable,
            args,
            cwd: cwd.to_string_lossy().to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
            status: "running".into(),
            kind,
            ended_at: None,
            exit_code: None,
        };
        self.storage.session_started(&session)?;

        let child = Arc::new(Mutex::new(child));
        let ended = Arc::new(AtomicBool::new(false));
        let session_state = Arc::new(Mutex::new(session.clone()));
        let handle = Arc::new(TerminalHandle {
            session: session_state.clone(),
            master: Mutex::new(Some(pair.master)),
            writer: Mutex::new(writer),
            child: child.clone(),
            ended: ended.clone(),
        });
        self.sessions
            .lock()
            .insert(session_id.clone(), handle.clone());

        let storage = self.storage.clone();
        std::thread::Builder::new()
            .name(format!("clicove-pty-{session_id}"))
            .spawn(move || {
                let mut buffer = [0_u8; 8192];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(size) => {
                            let output = TerminalOutput {
                                session_id: session_id.clone(),
                                data: String::from_utf8_lossy(&buffer[..size]).to_string(),
                            };
                            let _ = app.emit("terminal-output", output);
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
                let exit_code = child
                    .lock()
                    .wait()
                    .ok()
                    .map(|status| status.exit_code() as i32);
                ended.store(true, Ordering::SeqCst);
                {
                    let mut session = session_state.lock();
                    session.status = "exited".into();
                    session.ended_at = Some(chrono::Utc::now().to_rfc3339());
                    session.exit_code = exit_code;
                }
                let _ = storage.session_ended(&session_id, exit_code);
                let _ = app.emit(
                    "terminal-exit",
                    TerminalExit {
                        session_id,
                        exit_code,
                    },
                );
            })
            .map_err(|error| error.to_string())?;

        // ConPTY keeps its output pipe open until the master is closed, even after
        // cmd.exe exits. Close it while the reader is still draining the pipe.
        #[cfg(windows)]
        std::thread::spawn(move || loop {
            let finished = !matches!(handle.child.lock().try_wait(), Ok(None));
            if finished {
                drop(handle.master.lock().take());
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        });

        Ok(session)
    }

    pub fn write(&self, session_id: &str, data: &str) -> Result<(), String> {
        let handle = self.handle(session_id)?;
        if handle.ended.load(Ordering::SeqCst) {
            return Err("终端会话已经结束".into());
        }
        let mut writer = handle.writer.lock();
        writer
            .write_all(data.as_bytes())
            .and_then(|_| writer.flush())
            .map_err(|error| error.to_string())
    }

    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let handle = self.handle(session_id)?;
        let result = handle
            .master
            .lock()
            .as_ref()
            .ok_or_else(|| "终端会话已经结束".to_string())?
            .resize(PtySize {
                rows: rows.clamp(5, 300),
                cols: cols.clamp(20, 500),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| error.to_string());
        result
    }

    pub fn terminate(&self, session_id: &str) -> Result<(), String> {
        let handle = self.handle(session_id)?;
        if !handle.ended.load(Ordering::SeqCst) {
            handle
                .child
                .lock()
                .kill()
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn close(&self, session_id: &str) -> Result<(), String> {
        if let Some(handle) = self.sessions.lock().remove(session_id) {
            if !handle.ended.load(Ordering::SeqCst) {
                let _ = handle.child.lock().kill();
            }
            Ok(())
        } else {
            Err("未找到终端会话".into())
        }
    }

    pub fn list(&self) -> Vec<TerminalSession> {
        self.sessions
            .lock()
            .values()
            .map(|handle| {
                let mut session = handle.session.lock().clone();
                if handle.ended.load(Ordering::SeqCst) {
                    session.status = "exited".into();
                    session.ended_at = Some(chrono::Utc::now().to_rfc3339());
                }
                session
            })
            .collect()
    }

    pub fn rename(&self, session_id: &str, title: &str) -> Result<TerminalSession, String> {
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 80 || title.contains('\0') {
            return Err("会话名称不能为空且不能超过 80 个字符".into());
        }
        let handle = self.handle(session_id)?;
        let mut session = handle.session.lock();
        session.title = title.into();
        self.storage.session_renamed(session_id, title)?;
        Ok(session.clone())
    }

    pub fn with_idle_for_app_update<T>(
        &self,
        install: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _gate = self.spawn_gate.lock();
        if self.active_count() > 0 {
            return Err("请先结束正在运行的终端会话，再更新 CliCove。".into());
        }
        install()
    }

    pub fn active_count(&self) -> usize {
        self.sessions
            .lock()
            .values()
            .filter(|handle| !handle.ended.load(Ordering::SeqCst))
            .count()
    }

    pub fn terminate_all(&self) {
        for handle in self.sessions.lock().values() {
            if !handle.ended.load(Ordering::SeqCst) {
                let _ = handle.child.lock().kill();
            }
        }
    }

    fn handle(&self, session_id: &str) -> Result<Arc<TerminalHandle>, String> {
        self.sessions
            .lock()
            .get(session_id)
            .cloned()
            .ok_or_else(|| "未找到终端会话".into())
    }
}

fn pty_command(executable: &str, args: &[String]) -> CommandBuilder {
    #[cfg(windows)]
    {
        let extension = Path::new(executable)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if matches!(extension.as_str(), "cmd" | "bat") {
            let mut command = CommandBuilder::new("cmd.exe");
            command.args(["/D", "/S", "/C", executable]);
            command.args(args);
            return command;
        }
    }
    let mut command = CommandBuilder::new(executable);
    command.args(args);
    command
}

fn configure_terminal_environment(command: &mut CommandBuilder, login_path: &OsStr) {
    command.env("CLIHUB", "1");
    command.env("TERM", "xterm-256color");
    command.env("PATH", login_path);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::fs;
    use std::io::Read;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use tempfile::TempDir;

    #[test]
    fn portable_pty_captures_child_output() {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        #[cfg(unix)]
        let mut command = {
            let mut value = CommandBuilder::new("/bin/sh");
            value.args(["-c", "printf clicove-pty-ok"]);
            value
        };
        #[cfg(windows)]
        let mut command = {
            let mut value = CommandBuilder::new("cmd.exe");
            value.args(["/D", "/S", "/C", "echo clicove-pty-ok"]);
            value
        };
        command.env("TERM", "xterm-256color");
        let mut child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut output = String::new();
            let result = reader.read_to_string(&mut output).map(|_| output);
            let _ = send.send(result);
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                panic!("PTY child did not exit within 15 seconds");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert!(status.success());
        #[cfg(windows)]
        drop(pair.master);
        let output = receive
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("PTY output reader did not finish after child exit")
            .unwrap();
        assert!(output.contains("clicove-pty-ok"));
    }

    #[cfg(unix)]
    #[test]
    fn portable_pty_uses_login_path_for_env_shebangs() {
        let temp = TempDir::new().unwrap();
        let node = temp.path().join("node");
        let cli = temp.path().join("node-cli");
        fs::write(&node, "#!/bin/sh\nprintf 'pty-node-ok\\n'\n").unwrap();
        fs::write(&cli, "#!/usr/bin/env node\n").unwrap();
        fs::set_permissions(&node, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();

        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = pty_command(cli.to_str().unwrap(), &[]);
        configure_terminal_environment(&mut command, temp.path().as_os_str());
        let mut child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let mut output = String::new();
        pair.master
            .try_clone_reader()
            .unwrap()
            .read_to_string(&mut output)
            .unwrap();

        assert!(child.wait().unwrap().success());
        assert!(output.contains("pty-node-ok"));
    }
}
