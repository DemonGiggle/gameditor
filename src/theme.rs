use egui::{Color32, FontId, Rounding, Stroke, Style, TextStyle, Visuals};

// ── Palette ──────────────────────────────────────────────────────────────────

pub const BG_DARK: Color32 = Color32::from_rgb(24, 24, 32);
pub const BG_PANEL: Color32 = Color32::from_rgb(30, 30, 40);
pub const BG_WIDGET: Color32 = Color32::from_rgb(42, 42, 56);
pub const BG_WIDGET_HOVER: Color32 = Color32::from_rgb(52, 52, 68);
pub const BG_WIDGET_ACTIVE: Color32 = Color32::from_rgb(60, 60, 78);
pub const BG_STRIPE: Color32 = Color32::from_rgb(34, 34, 46);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(220, 220, 230);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 140, 160);

pub const ACCENT: Color32 = Color32::from_rgb(100, 140, 255);
pub const ACCENT_MUTED: Color32 = Color32::from_rgb(60, 80, 140);

pub const SUCCESS: Color32 = Color32::from_rgb(80, 200, 120);
pub const ERROR: Color32 = Color32::from_rgb(240, 80, 80);
pub const WARNING: Color32 = Color32::from_rgb(240, 180, 60);

pub const BORDER: Color32 = Color32::from_rgb(55, 55, 72);
pub const BORDER_FAINT: Color32 = Color32::from_rgb(45, 45, 58);

pub fn apply(ctx: &egui::Context) {
    let mut style = Style::default();

    // ── Typography ───────────────────────────────────────────────────────
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::proportional(18.0),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::proportional(13.5),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::proportional(11.5),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::proportional(13.5),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::monospace(13.0),
    );

    // ── Spacing ──────────────────────────────────────────────────────────
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(12.0);
    style.spacing.indent = 18.0;

    // ── Visuals ──────────────────────────────────────────────────────────
    let mut vis = Visuals::dark();

    vis.panel_fill = BG_PANEL;
    vis.window_fill = BG_PANEL;
    vis.faint_bg_color = BG_STRIPE;
    vis.extreme_bg_color = BG_DARK;

    vis.window_rounding = Rounding::same(8.0);
    vis.window_stroke = Stroke::new(1.0, BORDER);

    // Widgets
    vis.widgets.noninteractive.bg_fill = BG_PANEL;
    vis.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    vis.widgets.noninteractive.rounding = Rounding::same(4.0);
    vis.widgets.noninteractive.bg_stroke = Stroke::new(0.5, BORDER_FAINT);

    vis.widgets.inactive.bg_fill = BG_WIDGET;
    vis.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    vis.widgets.inactive.rounding = Rounding::same(5.0);
    vis.widgets.inactive.bg_stroke = Stroke::new(0.5, BORDER_FAINT);

    vis.widgets.hovered.bg_fill = BG_WIDGET_HOVER;
    vis.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    vis.widgets.hovered.rounding = Rounding::same(5.0);
    vis.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_MUTED);

    vis.widgets.active.bg_fill = BG_WIDGET_ACTIVE;
    vis.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    vis.widgets.active.rounding = Rounding::same(5.0);
    vis.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);

    vis.widgets.open.bg_fill = BG_WIDGET_ACTIVE;
    vis.widgets.open.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    vis.widgets.open.rounding = Rounding::same(5.0);
    vis.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT);

    vis.selection.bg_fill = ACCENT_MUTED;
    vis.selection.stroke = Stroke::new(1.0, ACCENT);

    vis.hyperlink_color = ACCENT;
    vis.override_text_color = Some(TEXT_PRIMARY);
    vis.striped = true;

    style.visuals = vis;
    ctx.set_style(style);
}
