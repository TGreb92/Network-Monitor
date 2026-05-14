//! # Presets Tab - Pack management and individual preset editing
//!
//! Top section: pack manager (load/rename/override/merge/delete/export/import).
//! Bottom section: individual preset editor (delegates to components::presets).

use eframe::egui;

use crate::core::preset_packs::{self, SavedPresetPack, builtin_packs};
use crate::ui::components::presets::{self, PresetEditorState};
use crate::ui::components::sidebar::SidebarState;

/// Pack management state (selection, renaming, import/export)
pub struct PackManagerState {
    pub selected: Option<usize>,
    pub new_name: String,
    pub show_save_form: bool,
    pub renaming: Option<usize>,
    pub rename_text: String,
    pub import_preview: Option<SavedPresetPack>,
    pub show_import_confirm: bool,
}

impl PackManagerState {
    pub fn new() -> Self {
        Self {
            selected: None,
            new_name: String::new(),
            show_save_form: false,
            renaming: None,
            rename_text: String::new(),
            import_preview: None,
            show_import_confirm: false,
        }
    }
}

/// State for the Presets tab
pub struct PresetsTabState {
    /// User-created preset packs
    pub custom_packs: Vec<SavedPresetPack>,
    /// Pack manager state (selection, renaming, import/export)
    pub pack_manager: PackManagerState,
    /// Individual preset editor state
    pub preset_editor: PresetEditorState,
    /// Local preset list for editing (independent from sidebar)
    pub editing_presets: Vec<crate::core::config::TargetPreset>,
    /// Selected index within the editing presets
    pub editing_selected: usize,
    /// Status message with expiry
    pub status: Option<(String, std::time::Instant)>,
}

impl PresetsTabState {
    pub fn from_packs_config(packs: &preset_packs::PacksConfig) -> Self {
        Self {
            custom_packs: packs.custom_packs.clone(),
            pack_manager: PackManagerState::new(),
            preset_editor: PresetEditorState::new(),
            editing_presets: crate::core::config::default_presets(),
            editing_selected: 0,
            status: None,
        }
    }

    /// Save current packs to disk
    fn save_packs(&self) {
        let config = preset_packs::PacksConfig {
            active_pack: String::new(),
            custom_packs: self.custom_packs.clone(),
        };
        let _ = preset_packs::save(&config);
    }

    /// Get all packs (built-in + custom) as a combined list
    pub fn all_packs(&self) -> Vec<SavedPresetPack> {
        let mut all = builtin_packs();
        all.extend(self.custom_packs.iter().cloned());
        all
    }
}

/// Render the Presets tab
pub fn render(ui: &mut egui::Ui, state: &mut PresetsTabState, _sidebar: &mut SidebarState) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.heading("📋 Presets");
            ui.add_space(8.0);

            render_preset_packs(ui, state);
            ui.add_space(8.0);

            presets::render(ui, &mut state.preset_editor, &mut state.editing_presets, &mut state.editing_selected);

            // Status message
            if let Some((text, when)) = &state.status {
                if when.elapsed().as_secs() < 5 {
                    ui.add_space(8.0);
                    let color = if text.starts_with('❌') {
                        egui::Color32::from_rgb(255, 120, 120)
                    } else {
                        egui::Color32::from_rgb(150, 200, 150)
                    };
                    ui.colored_label(color, text);
                }
            }
        });
}

fn render_preset_packs(ui: &mut egui::Ui, state: &mut PresetsTabState) {
    let header_id = ui.make_persistent_id("preset_packs_collapsible");
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), header_id, true)
        .show_header(ui, |ui| {
            ui.heading("📦 Preset Packs");
            let total = builtin_packs().len() + state.custom_packs.len();
            ui.label(format!("({})", total));
        })
        .body(|ui| {
            let actions = render_pack_lists(ui, state);
            process_pack_actions(state, actions);

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            render_pack_bottom_actions(ui, state);
            render_import_preview(ui, state);
        });
}

/// Deferred actions from pack list UI (avoids borrow conflicts)
struct PackActions {
    delete_idx: Option<usize>,
    update_idx: Option<usize>,
    merge_idx: Option<usize>,
    export_idx: Option<usize>,
    rename_start: Option<usize>,
    rename_confirm: bool,
}

fn render_pack_lists(ui: &mut egui::Ui, state: &mut PresetsTabState) -> PackActions {
    let builtins = builtin_packs();
    let builtin_count = builtins.len();

    let mut actions = PackActions {
        delete_idx: None, update_idx: None, merge_idx: None,
        export_idx: None, rename_start: None, rename_confirm: false,
    };

    render_builtin_packs(ui, state, &builtins, &mut actions);
    render_custom_packs(ui, state, builtin_count, &mut actions);

    actions
}

fn render_builtin_packs(
    ui: &mut egui::Ui,
    state: &mut PresetsTabState,
    builtins: &[SavedPresetPack],
    actions: &mut PackActions,
) {
    for (idx, pack) in builtins.iter().enumerate() {
        let is_selected = state.pack_manager.selected == Some(idx);
        ui.horizontal(|ui| {
            if ui.selectable_label(is_selected, format!("🔒 {}", pack.name)).clicked() {
                state.pack_manager.selected = Some(idx);
                state.editing_presets = pack.presets.clone();
                state.editing_selected = 0;
            }
            ui.label(egui::RichText::new(format!("({} presets)", pack.presets.len())).weak().small());
            if ui.small_button("📋").on_hover_text("Merge: add current presets not in this pack (by name)").clicked() {
                actions.merge_idx = Some(idx);
            }
            if ui.small_button("📤").on_hover_text("Export as JSON").clicked() {
                actions.export_idx = Some(idx);
            }
        });
    }
}

fn render_custom_packs(
    ui: &mut egui::Ui,
    state: &mut PresetsTabState,
    builtin_count: usize,
    actions: &mut PackActions,
) {
    let pack_count = state.custom_packs.len();
    for idx in 0..pack_count {
        let combined_idx = builtin_count + idx;
        let is_selected = state.pack_manager.selected == Some(combined_idx);
        let is_renaming = state.pack_manager.renaming == Some(idx);

        if is_renaming {
            render_renaming_row(ui, state, actions);
        } else {
            let name = state.custom_packs[idx].name.clone();
            let preset_count = state.custom_packs[idx].presets.len();
            ui.horizontal(|ui| {
                if ui.selectable_label(is_selected, format!("👤 {}", name)).clicked() {
                    state.pack_manager.selected = Some(combined_idx);
                    state.editing_presets = state.custom_packs[idx].presets.clone();
                    state.editing_selected = 0;
                }
                ui.label(egui::RichText::new(format!("({} presets)", preset_count)).weak().small());
                if ui.small_button("✏").on_hover_text("Rename").clicked() {
                    actions.rename_start = Some(idx);
                }
                if ui.small_button("🔄").on_hover_text("Override: replace with current presets").clicked() {
                    actions.update_idx = Some(idx);
                }
                if ui.small_button("📋").on_hover_text("Merge: add current presets not in pack (by name)").clicked() {
                    actions.merge_idx = Some(combined_idx);
                }
                if ui.small_button("📤").on_hover_text("Export as JSON").clicked() {
                    actions.export_idx = Some(combined_idx);
                }
                if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                    actions.delete_idx = Some(idx);
                }
            });
        }
    }
}

fn render_renaming_row(ui: &mut egui::Ui, state: &mut PresetsTabState, actions: &mut PackActions) {
    ui.horizontal(|ui| {
        ui.label("👤");
        ui.add(egui::TextEdit::singleline(&mut state.pack_manager.rename_text).desired_width(150.0));
        let can_save = !state.pack_manager.rename_text.trim().is_empty();
        if ui.add_enabled(can_save, egui::Button::new("✅").small()).on_hover_text("Confirm rename").clicked() {
            actions.rename_confirm = true;
        }
        if ui.small_button("✖").on_hover_text("Cancel rename").clicked() {
            state.pack_manager.renaming = None;
            state.pack_manager.rename_text.clear();
        }
    });
}

fn process_pack_actions(state: &mut PresetsTabState, actions: PackActions) {
    let builtin_count = builtin_packs().len();

    if let Some(idx) = actions.rename_start {
        state.pack_manager.renaming = Some(idx);
        state.pack_manager.rename_text = state.custom_packs[idx].name.clone();
    }
    if actions.rename_confirm {
        if let Some(idx) = state.pack_manager.renaming {
            if let Some(pack) = state.custom_packs.get_mut(idx) {
                pack.name = state.pack_manager.rename_text.trim().to_string();
            }
        }
        state.pack_manager.renaming = None;
        state.pack_manager.rename_text.clear();
        state.save_packs();
    }
    if let Some(idx) = actions.update_idx {
        state.custom_packs[idx].presets = state.editing_presets.clone();
        state.save_packs();
    }
    if let Some(combined_idx) = actions.merge_idx {
        process_merge(state, combined_idx, builtin_count);
    }
    if let Some(combined_idx) = actions.export_idx {
        export_pack_at(state, combined_idx);
    }
    if let Some(idx) = actions.delete_idx {
        state.custom_packs.remove(idx);
        if let Some(sel) = state.pack_manager.selected {
            let deleted = builtin_count + idx;
            if sel == deleted { state.pack_manager.selected = None; }
            else if sel > deleted { state.pack_manager.selected = Some(sel - 1); }
        }
        state.save_packs();
    }
}

fn process_merge(state: &mut PresetsTabState, combined_idx: usize, builtin_count: usize) {
    let merged = if combined_idx < builtin_count {
        let builtin = &builtin_packs()[combined_idx];
        let mut merged = builtin.presets.clone();
        let existing: std::collections::HashSet<String> = merged.iter().map(|p| p.name.clone()).collect();
        for p in &state.editing_presets {
            if !existing.contains(&p.name) { merged.push(p.clone()); }
        }
        state.custom_packs.push(SavedPresetPack {
            name: format!("{} (merged)", builtin.name),
            presets: merged.clone(),
        });
        state.pack_manager.selected = Some(builtin_count + state.custom_packs.len() - 1);
        merged
    } else {
        let custom_idx = combined_idx - builtin_count;
        let existing: std::collections::HashSet<String> =
            state.custom_packs[custom_idx].presets.iter().map(|p| p.name.clone()).collect();
        for p in state.editing_presets.clone() {
            if !existing.contains(&p.name) { state.custom_packs[custom_idx].presets.push(p); }
        }
        state.custom_packs[custom_idx].presets.clone()
    };
    state.editing_presets = merged;
    state.editing_selected = 0;
    state.save_packs();
}

fn export_pack_at(state: &mut PresetsTabState, combined_idx: usize) {
    let all = state.all_packs();
    if let Some(pack) = all.get(combined_idx) {
        let default_name = format!("{}.json", pack.name.replace(' ', "_").to_lowercase());
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Export preset pack")
            .set_file_name(&default_name)
            .add_filter("JSON", &["json"])
            .save_file()
        {
            match preset_packs::export_pack_json(pack, &path) {
                Ok(()) => state.status = Some((format!("✅ Exported \"{}\"", pack.name), std::time::Instant::now())),
                Err(e) => state.status = Some((format!("❌ Export failed: {}", e), std::time::Instant::now())),
            }
        }
    }
}

fn render_pack_bottom_actions(ui: &mut egui::Ui, state: &mut PresetsTabState) {
    let builtin_count = builtin_packs().len();

    if !state.pack_manager.show_save_form {
        ui.horizontal(|ui| {
            if ui.small_button("💾 Save current as new pack…").clicked() {
                state.pack_manager.show_save_form = true;
                state.pack_manager.new_name.clear();
            }
            if let Some(sel) = state.pack_manager.selected {
                if sel >= builtin_count {
                    let custom_idx = sel - builtin_count;
                    let pack_name = state.custom_packs.get(custom_idx)
                        .map(|p| p.name.clone())
                        .unwrap_or_default();
                    if ui.small_button(format!("🔄 Update \"{}\"", pack_name))
                        .on_hover_text("Replace selected pack with current presets")
                        .clicked()
                    {
                        state.custom_packs[custom_idx].presets = state.editing_presets.clone();
                        state.save_packs();
                    }
                }
            }
        });
    } else {
        render_save_pack_form(ui, state, builtin_count);
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.button("📤 Export pack").clicked() {
            export_selected_pack(state);
        }
        if ui.button("📥 Import pack").clicked() {
            import_pack(state);
        }
    });
}

fn render_save_pack_form(ui: &mut egui::Ui, state: &mut PresetsTabState, builtin_count: usize) {
    ui.horizontal(|ui| {
        ui.label("Pack name:");
        ui.add(egui::TextEdit::singleline(&mut state.pack_manager.new_name).desired_width(150.0));
        let can_save = !state.pack_manager.new_name.trim().is_empty();
        if ui.add_enabled(can_save, egui::Button::new("💾 Save")).clicked() {
            let new_idx = builtin_count + state.custom_packs.len();
            state.custom_packs.push(SavedPresetPack {
                name: state.pack_manager.new_name.trim().to_string(),
                presets: state.editing_presets.clone(),
            });
            state.pack_manager.selected = Some(new_idx);
            state.pack_manager.new_name.clear();
            state.pack_manager.show_save_form = false;
            state.save_packs();
        }
        if ui.button("Cancel").clicked() {
            state.pack_manager.new_name.clear();
            state.pack_manager.show_save_form = false;
        }
    });
}

fn render_import_preview(ui: &mut egui::Ui, state: &mut PresetsTabState) {
    if !state.pack_manager.show_import_confirm { return; }

    let preview_name = state.pack_manager.import_preview.as_ref().map(|p| p.name.clone());
    let preview_count = state.pack_manager.import_preview.as_ref().map(|p| p.presets.len());
    let preview_items: Vec<String> = state.pack_manager.import_preview.as_ref()
        .map(|p| p.presets.iter().map(|pr| format!("  • {} ({})", pr.name, pr.host)).collect())
        .unwrap_or_default();

    let Some(name) = preview_name else { return };
    let Some(count) = preview_count else { return };

    ui.add_space(4.0);
    ui.group(|ui| {
        ui.heading("📥 Import Preview");
        ui.label(format!("Pack: {}", name));
        ui.label(format!("Presets: {}", count));
        egui::ScrollArea::vertical()
            .id_salt("import_preview_list")
            .max_height(100.0)
            .show(ui, |ui| {
                for item in &preview_items {
                    ui.label(item);
                }
            });
        ui.horizontal(|ui| {
            if ui.button("✅ Import").clicked() {
                let pack = state.pack_manager.import_preview.take().unwrap();
                state.custom_packs.push(pack);
                state.pack_manager.show_import_confirm = false;
                state.save_packs();
            }
            if ui.button("✖ Cancel").clicked() {
                state.pack_manager.import_preview = None;
                state.pack_manager.show_import_confirm = false;
            }
        });
    });
}

fn export_selected_pack(state: &mut PresetsTabState) {
    if let Some(sel) = state.pack_manager.selected {
        export_pack_at(state, sel);
    } else {
        // No pack selected — export current editing presets
        let pack = SavedPresetPack {
            name: "Exported Pack".into(),
            presets: state.editing_presets.clone(),
        };
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Export preset pack")
            .add_filter("JSON", &["json"])
            .set_file_name("exported_pack.json")
            .save_file()
        {
            match preset_packs::export_pack_json(&pack, &path) {
                Ok(()) => state.status = Some(("✅ Exported".into(), std::time::Instant::now())),
                Err(e) => state.status = Some((format!("❌ Export failed: {}", e), std::time::Instant::now())),
            }
        }
    }
}

fn import_pack(state: &mut PresetsTabState) {
    if let Some(path) = rfd::FileDialog::new()
        .set_title("Import preset pack")
        .add_filter("JSON", &["json"])
        .pick_file()
    {
        match preset_packs::import_pack_json(&path) {
            Ok(pack) => {
                state.pack_manager.import_preview = Some(pack);
                state.pack_manager.show_import_confirm = true;
            }
            Err(e) => {
                state.status = Some((format!("❌ Import failed: {}", e), std::time::Instant::now()));
            }
        }
    }
}
