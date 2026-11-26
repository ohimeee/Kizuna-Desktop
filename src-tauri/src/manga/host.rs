//! Talks to the extension host (ext-host/, a small JVM process that runs Mihon extensions).
//! Started on first use, one JSON request per line on stdin, replies matched by id on stdout.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const START_TIMEOUT: Duration = Duration::from_secs(45);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

type Pending = Arc<Mutex<HashMap<u64, Sender<Result<Value, String>>>>>;

struct Running {
    child: Child,
    stdin: ChildStdin,
    pending: Pending,
}

/// Where the engine lives. Installed: `<app data>/manga-engine/{runtime,kizuna-ext-host.jar}`.
/// Development overrides: `KIZUNA_JAVA` (java.exe) and `KIZUNA_EXT_HOST_JAR`.
#[derive(Debug, Clone)]
pub struct EnginePaths {
    pub java: PathBuf,
    pub jar: PathBuf,
    pub data: PathBuf,
}

impl EnginePaths {
    pub fn resolve(engine_dir: &Path, data: PathBuf) -> Self {
        let java = std::env::var_os("KIZUNA_JAVA")
            .map(PathBuf::from)
            .unwrap_or_else(|| engine_dir.join("runtime").join("bin").join("java.exe"));
        let jar = std::env::var_os("KIZUNA_EXT_HOST_JAR")
            .map(PathBuf::from)
            .unwrap_or_else(|| engine_dir.join("kizuna-ext-host.jar"));
        Self { java, jar, data }
    }

    pub fn installed(&self) -> bool {
        self.java.is_file() && self.jar.is_file()
    }
}

pub struct ExtHost {
    paths: Mutex<EnginePaths>,
    running: Mutex<Option<Running>>,
    next_id: AtomicU64,
    /// Last few stderr lines, for error messages when the host dies.
    log: Arc<Mutex<Vec<String>>>,
}

impl ExtHost {
    pub fn new(paths: EnginePaths) -> Self {
        Self { paths: Mutex::new(paths), running: Mutex::new(None), next_id: AtomicU64::new(1), log: Default::default() }
    }

    pub fn paths(&self) -> EnginePaths {
        self.paths.lock().unwrap().clone()
    }

    pub fn is_running(&self) -> bool {
        let mut guard = self.running.lock().unwrap();
        match guard.as_mut() {
            Some(r) => matches!(r.child.try_wait(), Ok(None)),
            None => false,
        }
    }

    /// Sends a request and waits for its reply (starting the host if needed).
    pub fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T> {
        let rx = {
            let mut guard = self.running.lock().unwrap();
            if !guard.as_mut().is_some_and(|r| matches!(r.child.try_wait(), Ok(None))) {
                *guard = Some(self.start()?);
            }
            let running = guard.as_mut().unwrap();
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let (tx, rx) = channel();
            running.pending.lock().unwrap().insert(id, tx);
            let line = json!({ "id": id, "method": method, "params": params }).to_string();
            if let Err(e) = writeln!(running.stdin, "{line}").and_then(|_| running.stdin.flush()) {
                running.pending.lock().unwrap().remove(&id);
                bail!("manga engine stopped: {e}{}", self.recent_log());
            }
            rx
        };
        let value = rx
            .recv_timeout(REQUEST_TIMEOUT)
            .map_err(|_| anyhow!("the manga engine didn't answer in time{}", self.recent_log()))?
            .map_err(|e| anyhow!(e))?;
        serde_json::from_value(value).with_context(|| format!("unexpected reply to {method}"))
    }

    pub fn stop(&self) {
        if let Some(mut r) = self.running.lock().unwrap().take() {
            drop(r.stdin); // EOF: the host finishes in-flight work and exits
            std::thread::sleep(Duration::from_millis(300));
            let _ = r.child.kill();
            let _ = r.child.wait();
        }
    }

    fn recent_log(&self) -> String {
        let log = self.log.lock().unwrap();
        if log.is_empty() { String::new() } else { format!("\n{}", log.join("\n")) }
    }

    fn start(&self) -> Result<Running> {
        let paths = self.paths();
        if !paths.installed() {
            bail!("The manga add-on isn't installed. Turn it on in More → Manga.");
        }
        std::fs::create_dir_all(&paths.data)?;
        let mut cmd = Command::new(&paths.java);
        cmd.arg("-Xss8m")
            .arg("-XX:+UseSerialGC")
            // dex2jar output doesn't always pass the JVM's strict bytecode verifier ("Bad type
            // on operand stack" when it can't resolve a type while computing frames), though it
            // runs fine. Extensions are fully trusted code either way (as on Android), so the
            // verifier isn't a security boundary here. (A diagnostic option since JDK 25.)
            .arg("-XX:+UnlockDiagnosticVMOptions")
            .arg("-XX:-BytecodeVerificationRemote")
            .arg("-Xmx768m")
            .arg("--enable-native-access=ALL-UNNAMED")
            .arg("-Dfile.encoding=UTF-8")
            .arg("-jar")
            .arg(&paths.jar)
            .arg(&paths.data)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn().with_context(|| format!("starting {}", paths.java.display()))?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let pending: Pending = Default::default();

        // stderr → small ring buffer
        let log = Arc::clone(&self.log);
        log.lock().unwrap().clear();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let mut l = log.lock().unwrap();
                l.push(line);
                if l.len() > 20 {
                    l.remove(0);
                }
            }
        });

        // stdout → replies
        let (ready_tx, ready_rx) = channel::<Result<(), String>>();
        let pend = Arc::clone(&pending);
        std::thread::spawn(move || {
            let mut ready_tx = Some(ready_tx);
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
                if msg.get("event").and_then(Value::as_str) == Some("ready") {
                    if let Some(tx) = ready_tx.take() {
                        let _ = tx.send(Ok(()));
                    }
                    continue;
                }
                let Some(id) = msg.get("id").and_then(Value::as_u64) else { continue };
                if let Some(tx) = pend.lock().unwrap().remove(&id) {
                    let reply = match msg.get("error").and_then(Value::as_str) {
                        Some(e) => Err(e.to_string()),
                        None => Ok(msg.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    let _ = tx.send(reply);
                }
            }
            // Host exited: fail everything still waiting.
            for (_, tx) in pend.lock().unwrap().drain() {
                let _ = tx.send(Err("the manga engine stopped".into()));
            }
            if let Some(tx) = ready_tx.take() {
                let _ = tx.send(Err("the manga engine exited while starting".into()));
            }
        });

        match ready_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => Ok(Running { child, stdin, pending }),
            Ok(Err(e)) => {
                let _ = child.kill();
                std::thread::sleep(Duration::from_millis(200));
                bail!("{e}{}", self.recent_log())
            }
            Err(_) => {
                let _ = child.kill();
                bail!("the manga engine took too long to start{}", self.recent_log())
            }
        }
    }
}

impl Drop for ExtHost {
    fn drop(&mut self) {
        self.stop();
    }
}
