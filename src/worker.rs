use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use crate::scanner::{filter_scan, full_scan};
use crate::memory::write_bytes;
use crate::types::{Candidate, Pin, WorkerCmd, WorkerResult};

struct State {
    handle_raw: Option<usize>,
    candidates: Vec<Candidate>,
    pins: Vec<Pin>,
}

impl State {
    fn close_handle(&mut self) {
        #[cfg(windows)]
        if let Some(raw) = self.handle_raw.take() {
            use std::ffi::c_void;
            use windows::Win32::Foundation::{CloseHandle, HANDLE};
            unsafe {
                let _ = CloseHandle(HANDLE(raw as *mut c_void));
            }
        }
        #[cfg(not(windows))]
        {
            self.handle_raw = None;
        }
    }
}

pub fn run(cmd_rx: Receiver<WorkerCmd>, result_tx: Sender<WorkerResult>) {
    let mut state = State {
        handle_raw: None,
        candidates: Vec::new(),
        pins: Vec::new(),
    };

    loop {
        match cmd_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(cmd) => handle_cmd(cmd, &mut state, &result_tx),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Freeze loop: rewrite all enabled pins.
                if let Some(handle) = state.handle_raw {
                    for pin in state.pins.iter().filter(|p| p.enabled) {
                        let _ = write_bytes(handle, pin.address, &pin.value);
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    state.close_handle();
}

fn handle_cmd(cmd: WorkerCmd, state: &mut State, tx: &Sender<WorkerResult>) {
    match cmd {
        WorkerCmd::Attach(pid) => {
            state.close_handle();
            state.candidates.clear();
            state.pins.clear();

            #[cfg(windows)]
            {
                use windows::Win32::System::Threading::{
                    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION,
                    PROCESS_VM_READ, PROCESS_VM_WRITE,
                };
                let access = PROCESS_VM_READ
                    | PROCESS_VM_WRITE
                    | PROCESS_VM_OPERATION
                    | PROCESS_QUERY_INFORMATION;
                match unsafe { OpenProcess(access, false, pid) } {
                    Ok(h) => {
                        state.handle_raw = Some(h.0 as usize);
                        let _ = tx.send(WorkerResult::Attached(pid));
                    }
                    Err(e) => {
                        let _ = tx.send(WorkerResult::AttachFailed(e.to_string()));
                    }
                }
            }
            #[cfg(not(windows))]
            {
                let _ = tx.send(WorkerResult::AttachFailed("Not running on Windows".into()));
            }
        }

        WorkerCmd::Scan(target) => {
            let Some(handle) = state.handle_raw else {
                let _ = tx.send(WorkerResult::ScanError("No process attached".into()));
                return;
            };
            let results = full_scan(handle, &target);
            state.candidates = results.clone();
            let _ = tx.send(WorkerResult::ScanComplete(results));
        }

        WorkerCmd::Rescan(target) => {
            let Some(handle) = state.handle_raw else {
                let _ = tx.send(WorkerResult::ScanError("No process attached".into()));
                return;
            };
            let old = std::mem::take(&mut state.candidates);
            let results = filter_scan(handle, &old, &target);
            state.candidates = results.clone();
            let _ = tx.send(WorkerResult::ScanComplete(results));
        }

        WorkerCmd::Write { address, value } => {
            let Some(handle) = state.handle_raw else {
                return;
            };
            if write_bytes(handle, address, &value) {
                let _ = tx.send(WorkerResult::WriteOk);
            } else {
                let _ = tx.send(WorkerResult::WriteErr(format!(
                    "Failed to write to {:#018x}",
                    address
                )));
            }
        }

        WorkerCmd::PinAdd(pin) => {
            state.pins.push(pin);
        }

        WorkerCmd::PinRemove(id) => {
            state.pins.retain(|p| p.id != id);
        }

        WorkerCmd::PinToggle(id) => {
            if let Some(pin) = state.pins.iter_mut().find(|p| p.id == id) {
                pin.enabled = !pin.enabled;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::time::Duration;
    use crate::types::{Pin, WorkerCmd, WorkerResult};

    fn spawn_worker() -> (std::sync::mpsc::Sender<WorkerCmd>, std::sync::mpsc::Receiver<WorkerResult>) {
        let (cmd_tx, cmd_rx) = channel::<WorkerCmd>();
        let (result_tx, result_rx) = channel::<WorkerResult>();
        std::thread::spawn(move || run(cmd_rx, result_tx));
        (cmd_tx, result_rx)
    }

    fn recv(rx: &std::sync::mpsc::Receiver<WorkerResult>) -> WorkerResult {
        rx.recv_timeout(Duration::from_secs(5)).expect("worker did not respond in time")
    }

    // ── Attach ────────────────────────────────────────────────────────────

    #[test]
    #[cfg(not(windows))]
    fn attach_fails_on_non_windows() {
        let (tx, rx) = spawn_worker();
        tx.send(WorkerCmd::Attach(1234)).unwrap();
        assert!(matches!(recv(&rx), WorkerResult::AttachFailed(_)));
    }

    // ── Scan / Rescan without a handle ────────────────────────────────────

    #[test]
    fn scan_without_attach_returns_error() {
        let (tx, rx) = spawn_worker();
        tx.send(WorkerCmd::Scan(vec![0x01, 0x02, 0x03, 0x04])).unwrap();
        assert!(matches!(recv(&rx), WorkerResult::ScanError(_)));
    }

    #[test]
    fn rescan_without_attach_returns_error() {
        let (tx, rx) = spawn_worker();
        tx.send(WorkerCmd::Rescan(vec![0x01, 0x02, 0x03, 0x04])).unwrap();
        assert!(matches!(recv(&rx), WorkerResult::ScanError(_)));
    }

    // ── Pin management (no crash contract) ───────────────────────────────
    // These commands produce no response; we verify the worker stays alive
    // after receiving them by successfully sending another command after.

    #[test]
    fn pin_add_remove_toggle_do_not_crash() {
        let (tx, rx) = spawn_worker();

        let pin = Pin { id: 1, address: 0x1000, width: 4, value: vec![0; 4], enabled: true };
        tx.send(WorkerCmd::PinAdd(pin)).unwrap();
        tx.send(WorkerCmd::PinToggle(1)).unwrap();
        tx.send(WorkerCmd::PinRemove(1)).unwrap();

        // Worker is still alive: a subsequent Scan returns ScanError, not a channel error.
        tx.send(WorkerCmd::Scan(vec![0; 4])).unwrap();
        assert!(matches!(recv(&rx), WorkerResult::ScanError(_)));
    }

    #[test]
    fn pin_toggle_unknown_id_does_not_crash() {
        let (tx, rx) = spawn_worker();
        tx.send(WorkerCmd::PinToggle(9999)).unwrap();
        tx.send(WorkerCmd::Scan(vec![0; 4])).unwrap();
        assert!(matches!(recv(&rx), WorkerResult::ScanError(_)));
    }

    #[test]
    fn pin_remove_unknown_id_does_not_crash() {
        let (tx, rx) = spawn_worker();
        tx.send(WorkerCmd::PinRemove(9999)).unwrap();
        tx.send(WorkerCmd::Scan(vec![0; 4])).unwrap();
        assert!(matches!(recv(&rx), WorkerResult::ScanError(_)));
    }

    // ── Channel disconnect shuts down worker ──────────────────────────────

    #[test]
    fn worker_exits_when_sender_dropped() {
        let (cmd_tx, cmd_rx) = channel::<WorkerCmd>();
        let (result_tx, result_rx) = channel::<WorkerResult>();
        let handle = std::thread::spawn(move || run(cmd_rx, result_tx));
        drop(cmd_tx); // disconnect
        handle.join().expect("worker thread should exit cleanly");
        // result_rx disconnected too — no panic expected
        drop(result_rx);
    }
}
