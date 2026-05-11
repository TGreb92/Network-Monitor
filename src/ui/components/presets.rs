//! # Preset Manager — CRUD for target presets
//!
//! Collapsible list with add/edit/delete for named target presets.

use eframe::egui;

use crate::core::config::TargetPreset;
use crate::ui::components::sidebar::SidebarState;

/// Preset editor state (lives in ConfigState)
pub struct PresetEditorState {
    pub edit_name: String,
    pub edit_host: String,
    pub editing_index: Option<usize>,
    pub show_add_form: bool,
}

impl PresetEditorState {
    pub fn new() -> Self {
        Self {
            edit_name: String::new(),
            edit_host: String::new(),
            editing_index: None,
            show_add_form: false,
        }
    }
}

/// Render the collapsible preset manager
pub fn render(ui: &mut egui::Ui, editor: &mut PresetEditorState, sidebar: &mut SidebarState) {
    let header_id = ui.make_persistent_id("presets_collapsible");
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), header_id, false)
        .show_header(ui, |ui| {
            ui.heading("🎯 Target Presets");
            ui.label(format!("({})", sidebar.presets.len()));
        })
        .body(|ui| {
            render_list(ui, editor, sidebar);
            ui.add_space(4.0);
            render_form(ui, editor, sidebar);
        });
}

fn render_list(ui: &mut egui::Ui, editor: &mut PresetEditorState, sidebar: &mut SidebarState) {
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
                    for (idx, preset) in sidebar.presets.iter().enumerate() {
                        let is_selected = idx == sidebar.selected_preset;
                        if ui.selectable_label(is_selected, &preset.name).clicked() {
                            sidebar.selected_preset = idx;
                        }
                        ui.label(&preset.host);
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
        if sidebar.presets.len() > 1 {
            sidebar.presets.remove(idx);
            if sidebar.selected_preset >= sidebar.presets.len() {
                sidebar.selected_preset = sidebar.presets.len() - 1;
            }
        }
    }

    if let Some(idx) = start_edit_index {
        editor.edit_name = sidebar.presets[idx].name.clone();
        editor.edit_host = sidebar.presets[idx].host.clone();
        editor.editing_index = Some(idx);
    }
}

fn render_form(ui: &mut egui::Ui, editor: &mut PresetEditorState, sidebar: &mut SidebarState) {
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

    let can_save = !editor.edit_name.trim().is_empty() && !editor.edit_host.trim().is_empty();

    ui.horizontal(|ui| {
        if is_editing {
            if ui.add_enabled(can_save, egui::Button::new("💾 Update")).clicked() {
                if let Some(idx) = editor.editing_index {
                    sidebar.presets[idx] = TargetPreset {
                        name: editor.edit_name.trim().to_string(),
                        host: editor.edit_host.trim().to_string(),
                    };
                }
                clear_form(editor);
            }
        } else if ui.add_enabled(can_save, egui::Button::new("➕ Add")).clicked() {
            sidebar.presets.push(TargetPreset {
                name: editor.edit_name.trim().to_string(),
                host: editor.edit_host.trim().to_string(),
            });
            clear_form(editor);
        }
        if ui.button("Cancel").clicked() {
            clear_form(editor);
        }
    });
}

fn clear_form(editor: &mut PresetEditorState) {
    editor.edit_name.clear();
    editor.edit_host.clear();
    editor.editing_index = None;
    editor.show_add_form = false;
}
