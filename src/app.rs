use std::sync::mpsc::{channel, Receiver, Sender};

use crate::process::{enumerate_processes, ProcessInfo};
use crate::scanner::{decode_value, encode_value};
use crate::theme;
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
        self.scan_status = format!("Attaching to {} (PID {})...", name, pid);
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
        self.scan_status = if is_rescan { "Re-scanning...".into() } else { "Scanning...".into() };
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
        self.write_status = format!("Writing {} to {:#018x}...", val, address);
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

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Classify a status string for coloring.
fn status_color(s: &str) -> egui::Color32 {
    if s.contains("failed") || s.contains("error") || s.contains("Invalid") {
        theme::ERROR
    } else if s.contains("successful") || s.contains("Attached to") {
        theme::SUCCESS
    } else if s.ends_with("...") || s.contains("Scanning") || s.contains("Attaching") {
        theme::WARNING
    } else {
        theme::TEXT_SECONDARY
    }
}

/// Draw a styled section heading with an optional right-side widget.
fn section_heading(ui: &mut egui::Ui, icon: &str, title: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).size(16.0).color(theme::ACCENT));
        ui.label(egui::RichText::new(title).size(15.0).strong().color(theme::TEXT_PRIMARY));
    });
    ui.add_space(2.0);
}

/// Styled nav tab button that returns true if clicked.
fn nav_tab(ui: &mut egui::Ui, label: &str, active: bool, enabled: bool) -> bool {
    let text = egui::RichText::new(label).size(14.0);
    let text = if active {
        text.color(theme::ACCENT).strong()
    } else if enabled {
        text.color(theme::TEXT_PRIMARY)
    } else {
        text.color(theme::TEXT_SECONDARY)
    };

    let btn = egui::Button::new(text)
        .frame(false)
        .rounding(egui::Rounding::same(4.0));

    let resp = ui.add_enabled(enabled, btn);

    // Draw an underline indicator for the active tab
    if active {
        let rect = resp.rect;
        let painter = ui.painter();
        painter.line_segment(
            [
                egui::pos2(rect.left() + 2.0, rect.bottom()),
                egui::pos2(rect.right() - 2.0, rect.bottom()),
            ],
            egui::Stroke::new(2.0, theme::ACCENT),
        );
    }

    resp.clicked()
}

/// Accent-colored primary button.
fn primary_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    let text = egui::RichText::new(label).color(if enabled {
        egui::Color32::WHITE
    } else {
        theme::TEXT_SECONDARY
    });
    let btn = egui::Button::new(text)
        .fill(if enabled { theme::ACCENT } else { theme::BG_WIDGET })
        .rounding(egui::Rounding::same(5.0));
    ui.add_enabled(enabled, btn).clicked()
}

/// Subtle small action button.
fn action_button(ui: &mut egui::Ui, label: &str) -> bool {
    let text = egui::RichText::new(label).size(12.0).color(theme::ACCENT);
    let btn = egui::Button::new(text)
        .rounding(egui::Rounding::same(4.0))
        .stroke(egui::Stroke::new(0.5, theme::ACCENT_MUTED));
    ui.add(btn).clicked()
}

// ── Main update ──────────────────────────────────────────────────────────────

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_results();

        // Keep repainting while scanning or freezing so UI stays live.
        if self.scanning || !self.pins.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        // ── Top navigation bar ───────────────────────────────────────────
        egui::TopBottomPanel::top("nav")
            .frame(egui::Frame::none().fill(theme::BG_DARK).inner_margin(egui::Margin {
                left: 16.0,
                right: 16.0,
                top: 8.0,
                bottom: 6.0,
            }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // App title
                    ui.label(
                        egui::RichText::new("Game Editor")
                            .size(15.0)
                            .strong()
                            .color(theme::ACCENT),
                    );
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Tabs
                    if nav_tab(ui, "Processes", self.page == Page::Processes, true) {
                        self.page = Page::Processes;
                        self.processes = enumerate_processes();
                    }
                    ui.add_space(4.0);
                    let scan_available = self.attached_pid.is_some();
                    if nav_tab(ui, "Scan", self.page == Page::Scan, scan_available) {
                        self.page = Page::Scan;
                    }

                    // Right side: attached process info + spinner
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.scanning {
                            ui.label(
                                egui::RichText::new("Scanning...")
                                    .size(12.0)
                                    .color(theme::WARNING),
                            );
                            ui.spinner();
                        }

                        if let Some(pid) = self.attached_pid {
                            ui.label(
                                egui::RichText::new(format!("PID {}", pid))
                                    .size(12.0)
                                    .color(theme::TEXT_SECONDARY),
                            );
                            ui.label(
                                egui::RichText::new(&self.attached_name)
                                    .size(13.0)
                                    .strong()
                                    .color(theme::SUCCESS),
                            );
                            ui.label(
                                egui::RichText::new("\u{2B24}")
                                    .size(8.0)
                                    .color(theme::SUCCESS),
                            );
                        }
                    });
                });
            });

        // ── Dispatch to page ─────────────────────────────────────────────
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

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme::BG_PANEL).inner_margin(egui::Margin::same(16.0)))
            .show(ctx, |ui| {
                section_heading(ui, "\u{1F5A5}", "Processes");

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Filter")
                            .size(13.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                    let filter_edit = egui::TextEdit::singleline(&mut self.proc_filter)
                        .desired_width(220.0)
                        .hint_text("Type to search...");
                    ui.add(filter_edit);
                    ui.add_space(4.0);
                    if primary_button(ui, "Refresh", true) {
                        self.processes = enumerate_processes();
                    }
                });

                ui.add_space(6.0);

                // Separator line
                let rect = ui.available_rect_before_wrap();
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.left(), rect.top()),
                        egui::pos2(rect.right(), rect.top()),
                    ],
                    egui::Stroke::new(1.0, theme::BORDER_FAINT),
                );
                ui.add_space(6.0);

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
                        .min_col_width(100.0)
                        .spacing(egui::vec2(12.0, 5.0))
                        .show(ui, |ui| {
                            // Header
                            ui.label(
                                egui::RichText::new("Name")
                                    .size(12.0)
                                    .strong()
                                    .color(theme::TEXT_SECONDARY),
                            );
                            ui.label(
                                egui::RichText::new("PID")
                                    .size(12.0)
                                    .strong()
                                    .color(theme::TEXT_SECONDARY),
                            );
                            ui.label(egui::RichText::new("").size(12.0));
                            ui.end_row();

                            for p in &filtered {
                                let active = self.attached_pid == Some(p.pid);
                                if active {
                                    ui.label(
                                        egui::RichText::new(&p.name)
                                            .strong()
                                            .color(theme::SUCCESS),
                                    );
                                } else {
                                    ui.label(&p.name);
                                }
                                ui.monospace(
                                    egui::RichText::new(p.pid.to_string())
                                        .color(theme::TEXT_SECONDARY),
                                );
                                if active {
                                    ui.label(
                                        egui::RichText::new("attached")
                                            .size(11.0)
                                            .color(theme::SUCCESS),
                                    );
                                } else if action_button(ui, "Attach") {
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
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(&self.scan_status)
                            .size(12.0)
                            .color(status_color(&self.scan_status)),
                    );
                }
            });
    }
}

// ── Scan page ────────────────────────────────────────────────────────────────

impl App {
    fn show_scan(&mut self, ctx: &egui::Context) {
        // ── Pin panel (bottom) ───────────────────────────────────────────
        egui::TopBottomPanel::bottom("pins_panel")
            .min_height(100.0)
            .resizable(true)
            .frame(
                egui::Frame::none()
                    .fill(theme::BG_DARK)
                    .inner_margin(egui::Margin { left: 16.0, right: 16.0, top: 10.0, bottom: 10.0 })
                    .stroke(egui::Stroke::new(1.0, theme::BORDER_FAINT)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    section_heading(ui, "\u{2744}", "Frozen Values");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("{} pinned", self.pins.len()))
                                .size(11.0)
                                .color(theme::TEXT_SECONDARY),
                        );
                    });
                });

                let mut remove_id: Option<u64> = None;
                let mut toggle_id: Option<u64> = None;

                egui::ScrollArea::vertical()
                    .id_salt("pin_scroll")
                    .show(ui, |ui| {
                        if self.pins.is_empty() {
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new("No frozen values. Pin a candidate from the scan results above.")
                                    .size(12.0)
                                    .color(theme::TEXT_SECONDARY),
                            );
                        } else {
                            egui::Grid::new("pin_grid")
                                .num_columns(5)
                                .striped(true)
                                .spacing(egui::vec2(12.0, 4.0))
                                .show(ui, |ui| {
                                    // Header
                                    for hdr in &["Address", "Value", "Width", "Enabled", ""] {
                                        ui.label(
                                            egui::RichText::new(*hdr)
                                                .size(12.0)
                                                .strong()
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                    }
                                    ui.end_row();

                                    for pin in &self.pins {
                                        ui.monospace(
                                            egui::RichText::new(format!("{:#018x}", pin.address))
                                                .color(theme::ACCENT),
                                        );
                                        ui.monospace(decode_value(&pin.value).to_string());
                                        ui.label(
                                            egui::RichText::new(format!("u{}", pin.width * 8))
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        let mut en = pin.enabled;
                                        if ui.checkbox(&mut en, "").changed() {
                                            toggle_id = Some(pin.id);
                                        }
                                        if action_button(ui, "Remove") {
                                            remove_id = Some(pin.id);
                                        }
                                        ui.end_row();
                                    }
                                });
                        }
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

        // ── Main scan panel (center) ─────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme::BG_PANEL).inner_margin(egui::Margin::same(16.0)))
            .show(ctx, |ui| {
                // ── Scan controls ────────────────────────────────────────
                section_heading(ui, "\u{1F50D}", "Scan");

                egui::Frame::none()
                    .fill(theme::BG_DARK)
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Value")
                                    .size(13.0)
                                    .color(theme::TEXT_SECONDARY),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut self.scan_value_str)
                                    .desired_width(130.0)
                                    .hint_text("Enter integer"),
                            );

                            ui.add_space(8.0);

                            ui.label(
                                egui::RichText::new("Width")
                                    .size(13.0)
                                    .color(theme::TEXT_SECONDARY),
                            );
                            egui::ComboBox::from_id_salt("width_combo")
                                .selected_text(format!("u{}", self.scan_width * 8))
                                .width(60.0)
                                .show_ui(ui, |ui| {
                                    for &w in &[1u8, 2, 4, 8] {
                                        ui.selectable_value(
                                            &mut self.scan_width,
                                            w,
                                            format!("u{}", w * 8),
                                        );
                                    }
                                });

                            ui.add_space(12.0);

                            let can_scan = !self.scanning
                                && self.attached_pid.is_some()
                                && self.scan_value_str.trim().parse::<u64>().is_ok();

                            if primary_button(ui, "First Scan", can_scan) {
                                self.do_scan(false);
                            }
                            if primary_button(
                                ui,
                                "Next Scan",
                                can_scan && !self.candidates.is_empty(),
                            ) {
                                self.do_scan(true);
                            }

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(4.0);

                            ui.label(
                                egui::RichText::new(format!("{} results", self.candidates.len()))
                                    .size(13.0)
                                    .strong()
                                    .color(if self.candidates.is_empty() {
                                        theme::TEXT_SECONDARY
                                    } else {
                                        theme::ACCENT
                                    }),
                            );
                        });

                        if !self.scan_status.is_empty() {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(&self.scan_status)
                                    .size(12.0)
                                    .color(status_color(&self.scan_status)),
                            );
                        }
                    });

                ui.add_space(8.0);

                // ── Write controls ───────────────────────────────────────
                egui::Frame::none()
                    .fill(theme::BG_DARK)
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Write value")
                                    .size(13.0)
                                    .color(theme::TEXT_SECONDARY),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut self.write_value_str)
                                    .desired_width(130.0)
                                    .hint_text("Value to write"),
                            );
                            if !self.write_status.is_empty() {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(&self.write_status)
                                        .size(12.0)
                                        .color(status_color(&self.write_status)),
                                );
                            }
                        });
                    });

                ui.add_space(10.0);

                // ── Candidate table ──────────────────────────────────────
                const DISPLAY_LIMIT: usize = 2000;
                let total = self.candidates.len();

                let mut write_action: Option<(u64, u8)> = None;
                let mut pin_idx: Option<usize> = None;

                egui::ScrollArea::vertical()
                    .id_salt("cand_scroll")
                    .show(ui, |ui| {
                        if self.candidates.is_empty() && !self.scanning {
                            ui.add_space(24.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("No scan results yet")
                                        .size(14.0)
                                        .color(theme::TEXT_SECONDARY),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(
                                        "Enter a value above and click First Scan to begin.",
                                    )
                                    .size(12.0)
                                    .color(theme::TEXT_SECONDARY),
                                );
                            });
                        } else {
                            egui::Grid::new("cand_grid")
                                .num_columns(5)
                                .striped(true)
                                .spacing(egui::vec2(12.0, 4.0))
                                .show(ui, |ui| {
                                    // Header
                                    for hdr in
                                        &["Address", "Value", "Width", "Pinned", "Actions"]
                                    {
                                        ui.label(
                                            egui::RichText::new(*hdr)
                                                .size(12.0)
                                                .strong()
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                    }
                                    ui.end_row();

                                    for (i, c) in
                                        self.candidates.iter().take(DISPLAY_LIMIT).enumerate()
                                    {
                                        ui.monospace(
                                            egui::RichText::new(format!(
                                                "{:#018x}",
                                                c.address
                                            ))
                                            .color(theme::ACCENT),
                                        );
                                        ui.monospace(decode_value(&c.value).to_string());
                                        ui.label(
                                            egui::RichText::new(format!("u{}", c.width * 8))
                                                .color(theme::TEXT_SECONDARY),
                                        );
                                        if c.pinned {
                                            ui.label(
                                                egui::RichText::new("pinned")
                                                    .size(11.0)
                                                    .color(theme::SUCCESS),
                                            );
                                        } else {
                                            ui.label(
                                                egui::RichText::new("-")
                                                    .color(theme::TEXT_SECONDARY),
                                            );
                                        }
                                        ui.horizontal(|ui| {
                                            if action_button(ui, "Write") {
                                                write_action = Some((c.address, c.width));
                                            }
                                            if action_button(ui, "Pin") {
                                                pin_idx = Some(i);
                                            }
                                        });
                                        ui.end_row();
                                    }

                                    if total > DISPLAY_LIMIT {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "... {} more not shown",
                                                total - DISPLAY_LIMIT
                                            ))
                                            .size(12.0)
                                            .color(theme::TEXT_SECONDARY),
                                        );
                                        ui.end_row();
                                    }
                                });
                        }
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
