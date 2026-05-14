//! # Preset Manager - CRUD for target presets
//!
//! Collapsible list with add/edit/delete for named target presets.
//! Operates on any `&mut Vec<TargetPreset>` + `&mut usize` (selected index).

use eframe::egui;

use crate::core::config::TargetPreset;
use crate::core::server_check::TestMode;

/// Preset editor state
pub struct PresetEditorState {
    pub edit_name: String,
    pub edit_host: String,
    pub edit_mode_tcp: bool,
    pub edit_port: String,
    pub edit_category: String,
    pub editing_index: Option<usize>,
    pub show_add_form: bool,
}

impl PresetEditorState {
    pub fn new() -> Self {
        Self {
            edit_name: String::new(),
            edit_host: String::new(),
            edit_mode_tcp: false,
            edit_port: "443".into(),
            edit_category: String::new(),
            editing_index: None,
            show_add_form: false,
        }
    }
}

/// Render the collapsible preset manager, operating on the given preset list
pub fn render(
    ui: &mut egui::Ui,
    editor: &mut PresetEditorState,
    presets: &mut Vec<TargetPreset>,
    selected: &mut usize,
) {
    let header_id = ui.make_persistent_id("presets_collapsible");
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), header_id, false)
        .show_header(ui, |ui| {
            ui.heading("🎯 Target Presets");
            ui.label(format!("({})", presets.len()));
        })
        .body(|ui| {
            render_list(ui, editor, presets, selected);
            ui.add_space(4.0);
            render_form(ui, editor, presets);
        });
}

fn render_list(
    ui: &mut egui::Ui,
    editor: &mut PresetEditorState,
    presets: &mut Vec<TargetPreset>,
    selected: &mut usize,
) {
    let mut delete_index: Option<usize> = None;
    let mut start_edit_index: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_salt("preset_list")
        .max_height(150.0)
        .show(ui, |ui| {
            egui::Grid::new("preset_grid")
                .striped(true)
                .min_col_width(20.0)
                .show(ui, |ui| {
                    for (idx, preset) in presets.iter().enumerate() {
                        let is_selected = idx == *selected;
                        if ui.selectable_label(is_selected, &preset.name).clicked() {
                            *selected = idx;
                        }
                        ui.label(&preset.host);
                        ui.label(egui::RichText::new(preset.mode.label()).small().weak());
                        if ui.small_button("✏").on_hover_text("Edit").clicked() {
                            start_edit_index = Some(idx);
                        }
                        if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                            delete_index = Some(idx);
                        }
                        ui.end_row();
                    }
                });
        });

    if let Some(idx) = delete_index {
        if presets.len() > 1 {
            presets.remove(idx);
            if *selected >= presets.len() {
                *selected = presets.len() - 1;
            }
        }
    }

    if let Some(idx) = start_edit_index {
        editor.edit_name = presets[idx].name.clone();
        editor.edit_host = presets[idx].host.clone();
        editor.edit_mode_tcp = presets[idx].mode.is_tcp();
        editor.edit_port = presets[idx].mode.port().to_string();
        editor.edit_category = presets[idx].category.clone();
        editor.editing_index = Some(idx);
    }
}

fn render_form(ui: &mut egui::Ui, editor: &mut PresetEditorState, presets: &mut Vec<TargetPreset>) {
    let is_editing = editor.editing_index.is_some();

    if !is_editing && !editor.show_add_form {
        if ui.small_button("➕ Add new preset").clicked() {
            editor.show_add_form = true;
        }
        return;
    }

    ui.label(if is_editing { "Edit Preset" } else { "New Preset" });

    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.add(egui::TextEdit::singleline(&mut editor.edit_name).desired_width(100.0));
        ui.label("Host:");
        ui.add(egui::TextEdit::singleline(&mut editor.edit_host).desired_width(120.0));
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut editor.edit_mode_tcp, "TCP mode");
        if editor.edit_mode_tcp {
            ui.label("Port:");
            ui.add(egui::TextEdit::singleline(&mut editor.edit_port).desired_width(50.0));
        }
        ui.label("Category:");
        ui.add(egui::TextEdit::singleline(&mut editor.edit_category).desired_width(80.0));
    });

    let can_save = !editor.edit_name.trim().is_empty() && !editor.edit_host.trim().is_empty();

    ui.horizontal(|ui| {
        if is_editing {
            if ui.add_enabled(can_save, egui::Button::new("💾 Update")).clicked() {
                if let Some(idx) = editor.editing_index {
                    presets[idx] = build_preset(editor);
                }
                clear_form(editor);
            }
        } else if ui.add_enabled(can_save, egui::Button::new("➕ Add")).clicked() {
            presets.push(build_preset(editor));
            clear_form(editor);
        }
        if ui.button("Cancel").clicked() {
            clear_form(editor);
        }
    });
}

fn build_preset(editor: &PresetEditorState) -> TargetPreset {
    let mode = if editor.edit_mode_tcp {
        let port = editor.edit_port.trim().parse::<u16>().unwrap_or(443);
        TestMode::Tcp { port }
    } else {
        TestMode::Icmp
    };
    TargetPreset {
        name: editor.edit_name.trim().to_string(),
        host: editor.edit_host.trim().to_string(),
        mode,
        category: editor.edit_category.trim().to_string(),
    }
}

fn clear_form(editor: &mut PresetEditorState) {
    editor.edit_name.clear();
    editor.edit_host.clear();
    editor.edit_mode_tcp = false;
    editor.edit_port = "443".into();
    editor.edit_category.clear();
    editor.editing_index = None;
    editor.show_add_form = false;
}
