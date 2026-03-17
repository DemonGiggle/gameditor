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
