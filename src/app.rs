use std::sync::mpsc::{channel, Receiver, Sender};

use crate::process::{enumerate_processes, ProcessInfo};
use crate::scanner::{decode_value, encode_value};
use crate::types::{Candidate, Pin, WorkerCmd, WorkerResult};

#[derive(Clone, PartialEq)]
enum Page {
    Processes,
    Scan,
}

pub struct App {
    // Navigation
    page: Page,

    // Process page
    processes: Vec<ProcessInfo>,
    proc_filter: String,

    // Attached state
    attached_pid: Option<u32>,
    attached_name: String,

    // Scan page
    scan_value_str: String,
    scan_width: u8,
    candidates: Vec<Candidate>,
    scanning: bool,
    scan_status: String,

    // Write
    write_value_str: String,
    write_status: String,

    // Pins (UI-authoritative copy)
    pins: Vec<Pin>,
    next_pin_id: u64,

    // Worker communication
    cmd_tx: Sender<WorkerCmd>,
    result_rx: Receiver<WorkerResult>,
}

impl App {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = channel::<WorkerCmd>();
        let (result_tx, result_rx) = channel::<WorkerResult>();
        std::thread::spawn(move || crate::worker::run(cmd_rx, result_tx));

        Self {
            page: Page::Processes,
            processes: vec![],
            proc_filter: String::new(),
            attached_pid: None,
            attached_name: String::new(),
            scan_value_str: String::new(),
            scan_width: 4,
            candidates: vec![],
            scanning: false,
            scan_status: String::new(),
            write_value_str: String::new(),
            write_status: String::new(),
            pins: vec![],
            next_pin_id: 1,
            cmd_tx,
            result_rx,
        }
    }

    fn send(&self, cmd: WorkerCmd) {
        let _ = self.cmd_tx.send(cmd);
    }

    fn attach(&mut self, pid: u32, name: &str) {
        self.attached_pid = None;
        self.attached_name = name.to_string();
        self.candidates.clear();
        self.pins.clear();
        self.scanning = false;
        self.scan_status = format!("Attaching to {} (PID {})…", name, pid);
        self.send(WorkerCmd::Attach(pid));
    }

    fn do_scan(&mut self, is_rescan: bool) {
        let val_str = self.scan_value_str.trim().to_string();
        let Ok(val) = val_str.parse::<u64>() else {
            self.scan_status = "Invalid value — enter a non-negative integer.".into();
            return;
        };
        let target = encode_value(val, self.scan_width);
        if target.is_empty() {
            self.scan_status = "Unsupported width.".into();
            return;
        }
        self.scanning = true;
        self.scan_status = if is_rescan { "Re-scanning…".into() } else { "Scanning…".into() };
        if is_rescan {
            self.send(WorkerCmd::Rescan(target));
        } else {
            self.candidates.clear();
            self.send(WorkerCmd::Scan(target));
        }
    }

    fn do_write(&mut self, address: u64, width: u8) {
        let val_str = self.write_value_str.trim().to_string();
        let Ok(val) = val_str.parse::<u64>() else {
            self.write_status = "Invalid value.".into();
            return;
        };
        let bytes = encode_value(val, width);
        self.write_status = format!("Writing {} to {:#018x}…", val, address);
        self.send(WorkerCmd::Write { address, value: bytes });
    }

    fn add_pin(&mut self, c: &Candidate) {
        let id = self.next_pin_id;
        self.next_pin_id += 1;
        // Use the write-value field if it contains a valid number, otherwise
        // fall back to the candidate's last-scanned value.
        let value = self
            .write_value_str
            .trim()
            .parse::<u64>()
            .ok()
            .map(|v| encode_value(v, c.width))
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| c.value.clone());
        let pin = Pin {
            id,
            address: c.address,
            width: c.width,
            value,
            enabled: true,
        };
        self.pins.push(pin.clone());
        self.send(WorkerCmd::PinAdd(pin));
        // Mark candidate as pinned
        for cand in self.candidates.iter_mut() {
            if cand.address == c.address {
                cand.pinned = true;
            }
        }
    }

    fn drain_results(&mut self) {
        while let Ok(result) = self.result_rx.try_recv() {
            match result {
                WorkerResult::Attached(pid) => {
                    self.attached_pid = Some(pid);
                    self.scan_status = format!("Attached to PID {}.", pid);
                    self.page = Page::Scan;
                }
                WorkerResult::AttachFailed(msg) => {
                    self.scan_status = format!("Attach failed: {}", msg);
                }
                WorkerResult::ScanComplete(results) => {
                    let n = results.len();
                    self.candidates = results;
                    self.scanning = false;
                    self.scan_status = format!("{} candidates found.", n);
                }
                WorkerResult::ScanError(msg) => {
                    self.scanning = false;
                    self.scan_status = format!("Scan error: {}", msg);
                }
                WorkerResult::WriteOk => {
                    self.write_status = "Write successful.".into();
                }
                WorkerResult::WriteErr(msg) => {
                    self.write_status = format!("Write failed: {}", msg);
                }
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_results();

        // Keep repainting while scanning or freezing so UI stays live.
        if self.scanning || !self.pins.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        // ── Top navigation bar ─────────────────────────────────────────────
        egui::TopBottomPanel::top("nav").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let on_proc = self.page == Page::Processes;
                let on_scan = self.page == Page::Scan;
                if ui.selectable_label(on_proc, "Processes").clicked() {
                    self.page = Page::Processes;
                    self.processes = enumerate_processes();
                }
                let scan_available = self.attached_pid.is_some();
                if ui
                    .add_enabled(scan_available, egui::SelectableLabel::new(on_scan, "Scan"))
                    .clicked()
                {
                    self.page = Page::Scan;
                }

                if let Some(pid) = self.attached_pid {
                    ui.separator();
                    ui.label(format!("◉ {} (PID {})", self.attached_name, pid));
                }

                if self.scanning {
                    ui.separator();
                    ui.spinner();
                    ui.label("Scanning…");
                }
            });
        });

        // ── Dispatch to page ───────────────────────────────────────────────
        let page = self.page.clone();
        match page {
            Page::Processes => self.show_processes(ctx),
            Page::Scan => self.show_scan(ctx),
        }
    }
}

// ── Process page ─────────────────────────────────────────────────────────────

impl App {
    fn show_processes(&mut self, ctx: &egui::Context) {
        if self.processes.is_empty() {
            self.processes = enumerate_processes();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Processes");
            ui.horizontal(|ui| {
                ui.label("Filter:");
                ui.text_edit_singleline(&mut self.proc_filter);
                if ui.button("Refresh").clicked() {
                    self.processes = enumerate_processes();
                }
            });
            ui.separator();

            let filter = self.proc_filter.to_lowercase();
            let filtered: Vec<ProcessInfo> = self
                .processes
                .iter()
                .filter(|p| filter.is_empty() || p.name.to_lowercase().contains(&filter))
                .cloned()
                .collect();

            let mut attach: Option<(u32, String)> = None;

            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("proc_grid")
                    .num_columns(3)
                    .striped(true)
                    .min_col_width(80.0)
                    .show(ui, |ui| {
                        ui.strong("Name");
                        ui.strong("PID");
                        ui.strong("");
                        ui.end_row();

                        for p in &filtered {
                            let active = self.attached_pid == Some(p.pid);
                            if active {
                                ui.strong(&p.name);
                            } else {
                                ui.label(&p.name);
                            }
                            ui.label(p.pid.to_string());
                            if ui.button("Attach").clicked() {
                                attach = Some((p.pid, p.name.clone()));
                            }
                            ui.end_row();
                        }
                    });
            });

            if let Some((pid, name)) = attach {
                self.attach(pid, &name);
            }

            if !self.scan_status.is_empty() {
                ui.separator();
                ui.label(&self.scan_status);
            }
        });
    }
}

// ── Scan page ────────────────────────────────────────────────────────────────

impl App {
    fn show_scan(&mut self, ctx: &egui::Context) {
        // ── Pin panel (bottom) ─────────────────────────────────────────────
        egui::TopBottomPanel::bottom("pins_panel")
            .min_height(120.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Frozen Values");

                let mut remove_id: Option<u64> = None;
                let mut toggle_id: Option<u64> = None;

                egui::ScrollArea::vertical()
                    .id_source("pin_scroll")
                    .show(ui, |ui| {
                        egui::Grid::new("pin_grid")
                            .num_columns(5)
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("Address");
                                ui.strong("Value");
                                ui.strong("Width");
                                ui.strong("Enabled");
                                ui.strong("");
                                ui.end_row();

                                for pin in &self.pins {
                                    ui.monospace(format!("{:#018x}", pin.address));
                                    ui.label(decode_value(&pin.value).to_string());
                                    ui.label(format!("u{}", pin.width * 8));
                                    let mut en = pin.enabled;
                                    if ui.checkbox(&mut en, "").changed() {
                                        toggle_id = Some(pin.id);
                                    }
                                    if ui.small_button("Remove").clicked() {
                                        remove_id = Some(pin.id);
                                    }
                                    ui.end_row();
                                }
                            });
                    });

                if let Some(id) = toggle_id {
                    if let Some(pin) = self.pins.iter_mut().find(|p| p.id == id) {
                        pin.enabled = !pin.enabled;
                    }
                    self.send(WorkerCmd::PinToggle(id));
                }
                if let Some(id) = remove_id {
                    self.pins.retain(|p| p.id != id);
                    self.send(WorkerCmd::PinRemove(id));
                }
            });

        // ── Main scan panel (center) ───────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            // Scan controls row
            ui.horizontal(|ui| {
                ui.label("Value:");
                ui.add(egui::TextEdit::singleline(&mut self.scan_value_str).desired_width(120.0));

                ui.label("Width:");
                egui::ComboBox::from_id_source("width_combo")
                    .selected_text(format!("u{}", self.scan_width * 8))
                    .show_ui(ui, |ui| {
                        for &w in &[1u8, 2, 4, 8] {
                            ui.selectable_value(&mut self.scan_width, w, format!("u{}", w * 8));
                        }
                    });

                let can_scan = !self.scanning
                    && self.attached_pid.is_some()
                    && self.scan_value_str.trim().parse::<u64>().is_ok();

                if ui
                    .add_enabled(can_scan, egui::Button::new("First Scan"))
                    .clicked()
                {
                    self.do_scan(false);
                }
                if ui
                    .add_enabled(
                        can_scan && !self.candidates.is_empty(),
                        egui::Button::new("Next Scan"),
                    )
                    .clicked()
                {
                    self.do_scan(true);
                }

                ui.separator();
                ui.label(format!("{} results", self.candidates.len()));
            });

            if !self.scan_status.is_empty() {
                ui.label(egui::RichText::new(&self.scan_status).small().weak());
            }

            ui.separator();

            // Write value row
            ui.horizontal(|ui| {
                ui.label("Write value:");
                ui.add(egui::TextEdit::singleline(&mut self.write_value_str).desired_width(120.0));
                if !self.write_status.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new(&self.write_status).small().weak());
                }
            });

            ui.separator();

            // Candidate table
            const DISPLAY_LIMIT: usize = 2000;
            let total = self.candidates.len();

            let mut write_action: Option<(u64, u8)> = None;
            let mut pin_idx: Option<usize> = None;

            egui::ScrollArea::vertical()
                .id_source("cand_scroll")
                .show(ui, |ui| {
                    egui::Grid::new("cand_grid")
                        .num_columns(5)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Address");
                            ui.strong("Value");
                            ui.strong("Width");
                            ui.strong("Pinned");
                            ui.strong("Actions");
                            ui.end_row();

                            for (i, c) in
                                self.candidates.iter().take(DISPLAY_LIMIT).enumerate()
                            {
                                ui.monospace(format!("{:#018x}", c.address));
                                ui.label(decode_value(&c.value).to_string());
                                ui.label(format!("u{}", c.width * 8));
                                ui.label(if c.pinned { "yes" } else { "no" });
                                ui.horizontal(|ui| {
                                    if ui.small_button("Write").clicked() {
                                        write_action = Some((c.address, c.width));
                                    }
                                    if ui.small_button("Pin").clicked() {
                                        pin_idx = Some(i);
                                    }
                                });
                                ui.end_row();
                            }

                            if total > DISPLAY_LIMIT {
                                ui.label(format!(
                                    "… {} more not shown",
                                    total - DISPLAY_LIMIT
                                ));
                                ui.end_row();
                            }
                        });
                });

            // Apply actions after the borrow-heavy rendering block
            if let Some((addr, width)) = write_action {
                self.do_write(addr, width);
            }
            if let Some(i) = pin_idx {
                if i < self.candidates.len() {
                    let c = self.candidates[i].clone();
                    self.add_pin(&c);
                }
            }
        });
    }
}
