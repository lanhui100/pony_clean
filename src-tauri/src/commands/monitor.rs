use pony_core::monitor::{self, Snapshot};
use std::sync::{Arc, RwLock, mpsc};
use std::thread;
use tauri::State;

pub struct MonitorState {
    pub snapshot: Arc<RwLock<Option<Snapshot>>>,
    pub cmd_tx: Option<mpsc::Sender<monitor::MonitorCommand>>,
    pub thread: Option<thread::JoinHandle<()>>,
}

impl Drop for MonitorState {
    fn drop(&mut self) {
        if let Some(ref tx) = self.cmd_tx {
            let _ = tx.send(monitor::MonitorCommand::Shutdown);
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

#[tauri::command]
pub fn get_processes(state: State<'_, MonitorState>) -> Result<Snapshot, String> {
    let guard = state
        .snapshot
        .read()
        .map_err(|e| format!("Monitor lock poisoned: {e}"))?;
    guard.clone().ok_or_else(|| "No data yet".into())
}

#[tauri::command]
pub async fn kill_process(
    pid: u32,
    name: String,
    state: State<'_, MonitorState>,
) -> Result<(), String> {
    let cmd_tx = state.cmd_tx.as_ref().ok_or("Monitor not initialized")?;
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    cmd_tx
        .send(monitor::MonitorCommand::Kill {
            pid,
            name,
            resp: resp_tx,
        })
        .map_err(|_| "Monitor channel disconnected".to_string())?;
    resp_rx
        .await
        .map_err(|_| "Kill response dropped".to_string())?
}
