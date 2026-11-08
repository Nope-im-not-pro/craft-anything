//! Settings: General (mode, theme, language), Engines (Ghidra, tool paths), AI, Builds (when a
//! build is good enough, generated art), Plugins (MCP servers), Safety.

use crate::i18n::{LANGUAGES, tr, trf};
use crate::state::SettingsTab;
use crate::theme::{self, ThemeKind, Tokens};
use crate::{CraftApp, Pick, icons, widgets};
use craft_anything_ai::BackendKind;
use craft_anything_engine::Mode;
use egui::{RichText, Ui, Vec2};
use std::path::PathBuf;

/// Draws the Settings modal and the content for its selected tab.
pub fn show(app: &mut CraftApp, ctx: &egui::Context) {
    if !app.state.settings_open {
        return;
    }
    let t = Tokens::get(ctx);
    let mut open = true;
    // A modal (dimmed backdrop, Esc or a click outside closes) with a full-size close button;
    // egui's window close box is 14 px.
    let resp = egui::Modal::new(egui::Id::new("settings-window"))
        .frame(egui::Frame::new().fill(t.card).corner_radius(t.r_lg()).inner_margin(egui::Margin::same(20)).stroke(egui::Stroke::new(1.0, t.card_border)))
        .show(ctx, |ui| {
            ui.set_width(680.0);
            ui.horizontal(|ui| {
                ui.label(theme::heading(tr("Settings"), theme::SECTION));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icons::button(ui, "x", 32.0, false, "Close").clicked() {
                        open = false;
                    }
                });
            });
            // An error (a save that failed) shows here too: the message bar is behind this window.
            if let Some(i) = app.state.toasts.iter().rposition(|(_, error)| *error) {
                let text = app.state.toasts[i].0.clone();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    icons::show(ui, "triangle-alert", 16.0, t.danger);
                    ui.add(egui::Label::new(RichText::new(text).color(t.danger)).wrap());
                    if icons::button(ui, "x", 24.0, false, "Dismiss").clicked() {
                        app.state.toasts.remove(i);
                    }
                });
            }
            ui.add_space(12.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(150.0);
                    for (tab, icon, title) in [
                        (SettingsTab::General, "settings", "General"),
                        (SettingsTab::Engines, "cpu", "Engines"),
                        (SettingsTab::Ai, "bot", "AI"),
                        (SettingsTab::Builds, "hammer", "Builds"),
                        (SettingsTab::Plugins, "puzzle", "Plugins"),
                        (SettingsTab::Safety, "shield", "Safety"),
                    ] {
                        let on = app.state.settings_tab == tab;
                        let (rect, r) = ui.allocate_exact_size(Vec2::new(150.0, 34.0), egui::Sense::click());
                        r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, tr(title)));
                        if on {
                            ui.painter().rect_filled(rect, t.radius, t.accent_soft);
                        } else if r.hovered() {
                            ui.painter().rect_filled(rect, t.radius, t.hover);
                        }
                        icons::paint(
                            ui,
                            egui::Rect::from_min_size(egui::pos2(rect.left() + 10.0, rect.center().y - 8.0), Vec2::splat(16.0)),
                            icon,
                            16.0,
                            if on { t.accent_ink } else { t.icon },
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + 36.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            tr(title),
                            theme::medium(theme::SMALL),
                            if on { t.text } else { t.text_dim },
                        );
                        if r.clicked() {
                            app.state.settings_tab = tab;
                            if tab == SettingsTab::Ai && app.detected.is_none() {
                                app.detect_backends();
                            }
                        }
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    // Exactly what's left of the window, with room for the scroll bar a long tab
                    // gets, so no tab can widen the window (#68).
                    let w = ui.available_width();
                    ui.set_width(w);
                    let content_w = w - ui.spacing().scroll.allocated_width();
                    // As tall as the window allows, so a tab's settings aren't cut off at the fold.
                    let h = (ctx.content_rect().height() - 220.0).clamp(280.0, 560.0);
                    // Not shrunk to the tab's content: that gave each tab its own width and put the
                    // scroll bar in a different place on every page.
                    let tab = app.state.settings_tab;
                    egui::ScrollArea::vertical().auto_shrink([false, false]).min_scrolled_height(h).max_height(h).max_width(w).show(ui, |ui| {
                        // The bar sits at the edge of the area, so the air between it and the
                        // settings is taken from the settings' side.
                        ui.set_max_width(content_w - 14.0);
                        match tab {
                            SettingsTab::General => general(app, ui),
                            SettingsTab::Engines => engines(app, ui),
                            SettingsTab::Ai => ai(app, ui),
                            SettingsTab::Builds => builds(app, ui),
                            SettingsTab::Plugins => plugins(app, ui),
                            SettingsTab::Safety => safety(app, ui),
                        }
                    });
                });
            });
        });
    if !open || resp.should_close() {
        app.state.settings_open = false;
    }
}

fn row(ui: &mut Ui, title: &str, sub: &str, add: impl FnOnce(&mut Ui)) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(220.0);
            ui.label(RichText::new(tr(title)).font(theme::medium(theme::SMALL)));
            if !sub.is_empty() {
                ui.add(egui::Label::new(RichText::new(tr(sub)).color(t.text_faint).size(theme::CAPTION)).wrap());
            }
        });
        add(ui);
    });
    ui.add_space(10.0);
}

fn general(app: &mut CraftApp, ui: &mut Ui) {
    row(ui, "Mode", "Simple: say what you want and it's handled for you. Advanced: every tool, the evidence and the logs.", |ui| {
        let sel = usize::from(app.settings.mode == Mode::Experienced);
        if let Some(i) = widgets::segmented(ui, &["Simple", "Advanced"], sel, 30.0) {
            app.set_mode(if i == 0 { Mode::Beginner } else { Mode::Experienced });
        }
    });
    let dir = app.settings.projects_dir();
    row(ui, "New projects go in", &dir.display().to_string(), |ui| {
        if widgets::secondary(ui, Some("folder-open"), "Change…").clicked() {
            app.pick(crate::Pick::ProjectsDir);
        }
    });
    row(ui, "Also list projects from", "Folders of projects, and projects started in a folder you picked. Removing one only takes it off the list.", |ui| {
        if widgets::secondary(ui, Some("plus"), "Add a folder…").clicked() {
            app.pick(crate::Pick::ExtraProjectsDir);
        }
    });
    let mut remove = None;
    for f in &app.settings.project_folders {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            crate::icons::show(ui, "folder", 14.0, Tokens::get(ui.ctx()).text_faint);
            ui.add(egui::Label::new(RichText::new(f.display().to_string()).font(theme::mono(12.0))).truncate());
            if crate::icons::button(ui, "x", 22.0, false, &trf("Stop listing {folder}", &[("folder", &f.display().to_string())])).clicked() {
                remove = Some(f.clone());
            }
        });
    }
    if let Some(f) = remove {
        app.remove_project_folder(&f);
    }
    row(ui, "When a folder you pick has files", "Whether they're the app to edit in place, or material the app starts from beside them.", |ui| {
        let current = crate::folders::EXISTING_CHOICES.iter().find(|(_, v)| *v == app.settings.existing_files).map_or("Ask each time", |(l, _)| *l);
        egui::ComboBox::from_id_salt("existing-files").icon(widgets::combo_icon).selected_text(tr(current)).show_ui(ui, |ui| {
            for (label, value) in crate::folders::EXISTING_CHOICES {
                if ui.selectable_label(app.settings.existing_files == value, tr(label)).clicked() {
                    app.settings.existing_files = value;
                    app.settings_changed();
                }
            }
        });
    });
    row(ui, "Theme", "", |ui| {
        let sel = usize::from(app.theme_kind() == ThemeKind::StudioLight);
        if let Some(i) = widgets::segmented(ui, &["Studio", "Studio Light"], sel, 30.0) {
            app.settings.theme = ThemeKind::ALL.get(i).copied().unwrap_or_default().id().into();
            app.settings_changed();
        }
    });
    // A choice of one isn't a choice: the row appears once there's a second language.
    if LANGUAGES.len() > 1 {
        language(app, ui);
    }
    row(ui, "Show the welcome again", "Runs the first-run steps next time.", |ui| {
        if widgets::secondary(ui, Some("refresh-cw"), "Reset").clicked() {
            app.settings.first_run_done = false;
            app.state.first_run = crate::state::FirstRunStep::Welcome;
            app.state.settings_open = false;
            app.settings_changed();
        }
    });
}

fn language(app: &mut CraftApp, ui: &mut Ui) {
    row(ui, "Language", "", |ui| {
        let selected = (!app.settings.language.trim().is_empty()).then(|| crate::i18n::language_from_tag(&app.settings.language).unwrap_or("en"));
        let current = LANGUAGES.iter().find(|l| Some(l.code) == selected).map(|l| l.name).unwrap_or("System");
        egui::ComboBox::from_id_salt("lang").icon(widgets::combo_icon).selected_text(tr(current)).show_ui(ui, |ui| {
            if ui.selectable_label(app.settings.language.trim().is_empty(), tr("System")).clicked() {
                app.settings.language.clear();
                crate::i18n::set_language("");
                app.settings_changed();
            }
            for l in LANGUAGES {
                if ui.selectable_label(selected == Some(l.code), l.name).clicked() {
                    app.settings.language = l.code.into();
                    crate::i18n::set_language(l.code);
                    app.settings_changed();
                }
            }
        });
    });
}

/// A path setting; `name` is what a screen reader calls it.
fn path_field(ui: &mut Ui, value: &mut Option<PathBuf>, name: &str, hint: &str) -> bool {
    // While typing, the text as typed: the path kept is trimmed, and showing that instead would
    // drop the space typed in "C:\Program Files" before the next letter.
    let typed = ui.next_auto_id().with("typed");
    let shown = value.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
    let mut s = ui.data(|d| d.get_temp::<String>(typed)).unwrap_or(shown);
    let r = ui.add(
        egui::TextEdit::singleline(&mut s)
            .hint_text(tr(hint))
            // Up to 240, leaving room for a Choose… button after it, so a row never widens the window (#68).
            .desired_width((ui.available_width() - 34.0).clamp(120.0, 240.0))
            .font(theme::mono(12.0))
            .frame(widgets::text_field_frame_compact(&Tokens::get(ui.ctx()))),
    );
    widgets::named(r.clone(), &tr(name));
    if r.has_focus() {
        ui.data_mut(|d| d.insert_temp(typed, s.clone()));
    } else {
        ui.data_mut(|d| d.remove::<String>(typed));
    }
    if r.changed() {
        *value = if s.trim().is_empty() { None } else { Some(PathBuf::from(s.trim())) };
        return true;
    }
    false
}

fn engines(app: &mut CraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    row(
        ui,
        "Clean room / Deep",
        "Clean room never reads the original's code, files or page source: it works from what you can see. Deep reads them with every tool, and with Ghidra when it's installed. Every finding records which was used.",
        |ui| {
            let sel = usize::from(app.settings.deep_mode);
            if let Some(i) = widgets::segmented(ui, &["Clean room", "Deep"], sel, 30.0) {
                app.set_deep(i == 1);
            }
        },
    );
    ui.add_space(12.0);
    row(ui, "Ghidra", "Adds its decompiler to Deep mode. Craft Anything can download and set it up for you.", |ui| {
        crate::engine_ui::ghidra_status(app, ui);
    });
    row(
        ui,
        "Memory for Ghidra",
        "More memory makes Ghidra's first look at a big program faster; leave some for the rest of the computer. Used from the next time Ghidra starts.",
        |ui| crate::engine_ui::ghidra_memory_setting(app, ui),
    );
    ui.label(RichText::new(crate::engine_ui::ghidra_memory_guide()).color(t.text_faint).size(theme::CAPTION));
    ui.add_space(8.0);
    row(ui, "Ghidra folder", "Leave empty to use the one Craft Anything sets up.", |ui| {
        if path_field(ui, &mut app.settings.engines.ghidra.install_dir, "Ghidra folder", "managed") {
            app.settings_changed();
        }
        if icons::button(ui, "folder-open", 26.0, false, "Choose…").clicked() {
            app.pick(Pick::GhidraDir);
        }
    });
    row(ui, "Java runtime", "Only for Ghidra. Empty = managed.", |ui| {
        if path_field(ui, &mut app.settings.engines.ghidra.java_home, "Java runtime", "managed") {
            app.settings_changed();
        }
    });
    ui.add_space(12.0);
    crate::engine_ui::settings_section(app, ui);
    ui.add_space(12.0);
    widgets::caption(ui, "Your own tool installs (optional)");
    ui.label(RichText::new(tr("Leave a path empty to use the managed install. We never need environment variables.")).color(t.text_faint).size(theme::CAPTION));
    ui.add_space(6.0);
    let mut changed = false;
    egui::Grid::new("tool-paths").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        // Names and purposes come from the engine table; the override list matches it (tested).
        for (id, v) in app.settings.engines.tools.entries_mut() {
            let (name, needed_for) = craft_anything_engines::engine(id).map(|e| (e.name, e.needed_for)).unwrap_or((id, ""));
            ui.label(RichText::new(tr(name)).size(theme::CAPTION)).on_hover_text(format!("{}\n{id}", tr(needed_for)));
            changed |= path_field(ui, v, name, "managed");
            ui.end_row();
        }
    });
    if changed {
        app.settings_changed();
    }
}

/// Keep both selectors in place while their model-specific choices load or fail. Loading is
/// scheduled after drawing, and a failed lookup stays visible until explicitly retried.
fn model_options_loading(ui: &mut Ui, backend: &str) {
    ui.add_enabled_ui(false, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("Context window"));
            egui::ComboBox::from_id_salt(("model-context", backend)).icon(widgets::combo_icon).selected_text(tr("Default")).show_ui(ui, |_| {});
        });
    });
}

/// Draw AI settings and model pickers, preserving drafts during contention and invalidating confirmed context changes.
fn ai(app: &mut CraftApp, ui: &mut Ui) {
    if app.agent_model_picker.as_ref().is_some_and(|p| app.hub.model_context(&p.context.backend.config.name).is_ok_and(|current| current != p.context)) {
        app.agent_model_picker = None;
    }
    let t = Tokens::get(ui.ctx());
    crate::engine_ui::assets_settings(app, ui);
    ui.add_space(16.0);
    widgets::caption(ui, "Your brains");
    ui.add_space(4.0);
    if app.backends.is_empty() {
        ui.label(RichText::new(tr("No AI is set up yet. The built-in engines still work; an AI adds plain-words answers.")).color(t.text_dim));
    }
    let (mut pick, mut builds, mut extracts, mut toggle, mut remove) = (None, None, None, None, None);
    let (mut choose_program, mut usual_program) = (None, None);
    let mut save_folder = None;
    // Taken out while the rows borrow the app, and put back after them.
    let mut folder_drafts = std::mem::take(&mut app.folder_inputs);
    let mut key_change = app.key_change.take();
    let (mut change_key_for, mut save_key) = (None, false);
    let (mut pick_model, mut chosen_model, mut close_picker) = (None, None, false);
    let mut chosen_effort = None;
    let (mut pick_agent_model, mut chosen_agent_model, mut remote_program) = (None, None, None);
    let (mut tune_models, mut save_tuning) = (Vec::new(), None);
    // The AI whose Remove was pressed once: asked again before its key is deleted.
    let confirm_id = egui::Id::new("remove-ai");
    let confirming: Option<String> = ui.data(|d| d.get_temp(confirm_id));
    for b in &app.backends {
        widgets::panel(&t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // The name and model on their own row (shortened, full text on hover), the buttons on a
            // row below that wraps: nothing overlaps however long the name or narrow the window.
            ui.horizontal(|ui| {
                icons::show(ui, "bot", 18.0, if b.enabled { t.accent } else { t.text_faint });
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(RichText::new(&b.name).font(theme::semibold(theme::SMALL))).truncate());
                    let model_in_use = craft_anything_ai::model_in_use(b, app.backend_options.get(&b.name));
                    let live_catalog = b.kind == BackendKind::AgentCli && craft_anything_ai::cli_agents::adapter_for(&b.model).has_model_catalog();
                    // An API's model has its own line below; an agent app is named by its `model`.
                    let what = if model_in_use.is_some() && b.kind != BackendKind::AgentCli { String::new() } else { format!(" · {}", b.model) };
                    let ssh = b.ssh_host.as_ref().map(|h| format!(" · ssh {h}")).unwrap_or_default();
                    ui.add(egui::Label::new(RichText::new(format!("{:?}{what}{ssh}", b.kind)).color(t.text_faint).size(theme::CAPTION)).truncate());
                    // An agent app on this computer: which program runs it, for one installed
                    // where it isn't looked for (#53).
                    if b.kind == BackendKind::AgentCli && b.ssh_host.as_deref().is_none_or(|h| h.trim().is_empty()) {
                        let program = app.backend_options.get(&b.name).and_then(|o| o.command.as_ref());
                        ui.horizontal_wrapped(|ui| {
                            // The links at the caption's size, so the line reads as one.
                            ui.style_mut().override_font_id = Some(egui::FontId::proportional(theme::CAPTION));
                            let text = program.map_or_else(|| tr("Program: found automatically"), |p| trf("Program: {path}", &[("path", p)]));
                            ui.add(egui::Label::new(RichText::new(text).color(t.text_faint).size(theme::CAPTION)).truncate());
                            if widgets::link(ui, "Choose…").clicked() {
                                choose_program = Some(b.name.clone());
                            }
                            if program.is_some() && widgets::link(ui, "Find it automatically").clicked() {
                                usual_program = Some(b.name.clone());
                            }
                        });
                    }
                    // The folder it keeps its sign-in in (Claude Code's CLAUDE_CONFIG_DIR), typed
                    // so `~/` and hidden folders work, here or over SSH: another folder is another
                    // account. Saved on Enter or when the field is left; empty is its usual one.
                    if craft_anything_ai::folder_var(b).is_some() {
                        let saved = app.backend_options.get(&b.name).and_then(|o| o.config_dir.clone()).unwrap_or_default();
                        let id = egui::Id::new(("agent-folder", &b.name));
                        // What was typed stays until it's saved (or refused, so it can be fixed).
                        if folder_drafts.get(&b.name).is_some_and(|d| d.trim() == saved) && !ui.memory(|m| m.has_focus(id)) {
                            folder_drafts.remove(&b.name);
                        }
                        let mut text = folder_drafts.get(&b.name).cloned().unwrap_or_else(|| saved.clone());
                        ui.add_space(4.0);
                        let label = ui.label(RichText::new(tr("Claude Code folder (CLAUDE_CONFIG_DIR)")).size(theme::CAPTION).color(t.text_dim));
                        ui.label(
                            RichText::new(tr("Its own sign-in, settings and conversations: another folder is another account. Empty: its usual folder."))
                                .color(t.text_faint)
                                .size(theme::CAPTION),
                        );
                        let r = ui
                            .add(
                                egui::TextEdit::singleline(&mut text)
                                    .id(id)
                                    .hint_text("~/.claude")
                                    .desired_width(260.0)
                                    .frame(widgets::text_field_frame_compact(&t)),
                            )
                            .labelled_by(label.id);
                        if r.changed() {
                            folder_drafts.insert(b.name.clone(), text.clone());
                        }
                        if r.lost_focus() && text.trim() != saved {
                            save_folder = Some((b.name.clone(), text.trim().to_string()));
                        }
                        ui.add_space(4.0);
                    }
                    // How hard it thinks: the Claude API and Claude Code take every Claude level.
                    let effort = craft_anything_ai::effort_in_use(b, app.backend_options.get(&b.name));
                    if b.kind == BackendKind::AgentCli && b.ssh_host.as_deref().is_some_and(|h| !h.trim().is_empty()) {
                        let id = egui::Id::new(("remote-agent-program", &b.name));
                        let mut program = ui
                            .data_mut(|d| d.get_temp::<String>(id))
                            .unwrap_or_else(|| app.backend_options.get(&b.name).and_then(|o| o.command.clone()).unwrap_or_default());
                        ui.horizontal_wrapped(|ui| {
                            ui.label(tr("Remote program"));
                            ui.add(egui::TextEdit::singleline(&mut program).hint_text(&b.model).desired_width(200.0));
                            if widgets::link(ui, "Apply").clicked() {
                                remote_program = Some((b.name.clone(), program.clone()));
                            }
                        });
                        ui.data_mut(|d| d.insert_temp(id, program));
                    }
                    // Which model it uses, and the ones it can be set to (#123).
                    if live_catalog {
                        let options = app.backend_options.get(&b.name);
                        let current = options
                            .and_then(|o| o.cli_model.as_deref())
                            .map(str::to_string)
                            .unwrap_or_else(|| if b.model == "opencode" { tr("OpenCode default") } else { tr("Model default") });
                        ui.label(trf("Model: {model}", &[("model", &current)]));
                        let thinking = options.and_then(|o| o.cli_thinking).map(|d| d.title().to_string()).unwrap_or_else(|| tr("Model default"));
                        ui.label(format!("{}: {thinking}", tr("Thinking depth")));
                        if let Some(picker) = app.agent_model_picker.as_mut().filter(|p| p.context.backend.config.name == b.name) {
                            let previous = picker.model.clone();
                            let apply = crate::builders::agent_model_picker(ui, picker);
                            if picker.model != previous && craft_anything_ai::cli_agents::adapter_for(&b.model).model_catalog_depends_on_selection() {
                                pick_agent_model = Some(b.name.clone());
                            } else if apply {
                                chosen_agent_model = Some((b.name.clone(), picker.model.clone(), picker.thinking));
                            }
                            ui.horizontal(|ui| {
                                if widgets::link(ui, "Reload models").clicked() {
                                    pick_agent_model = Some(b.name.clone());
                                }
                                if widgets::link(ui, "Cancel").clicked() {
                                    close_picker = true;
                                }
                            });
                        } else if widgets::link(ui, "Change…").clicked() {
                            pick_agent_model = Some(b.name.clone());
                        }
                    } else if let Some(current) = model_in_use {
                        ui.horizontal_wrapped(|ui| {
                            ui.style_mut().override_font_id = Some(egui::FontId::proportional(theme::CAPTION));
                            ui.add(
                                egui::Label::new(RichText::new(trf("Model: {model}", &[("model", &current)])).color(t.text_faint).size(theme::CAPTION))
                                    .truncate(),
                            );
                            match app.model_picker.as_ref().filter(|(n, _)| *n == b.name) {
                                None => {
                                    if widgets::link(ui, "Change…").clicked() {
                                        pick_model = Some(b.name.clone());
                                    }
                                }
                                Some((_, None)) => {
                                    ui.add(crate::widgets::Spinner::new().size(theme::CAPTION).color(t.accent));
                                    ui.label(RichText::new(tr("Asking for its models…")).color(t.text_faint).size(theme::CAPTION));
                                }
                                Some((_, Some(models))) => {
                                    egui::ComboBox::from_id_salt(("model", &b.name)).icon(widgets::combo_icon).selected_text(&current).show_ui(ui, |ui| {
                                        for m in models {
                                            if ui.selectable_label(*m == current, m).clicked() && *m != current {
                                                chosen_model = Some((b.name.clone(), m.clone()));
                                            }
                                        }
                                    });
                                    // A model the list doesn't offer: a newer one, a variant such as
                                    // `opusplan`, or one another harness serves (#470).
                                    let id = egui::Id::new(("other-model", &b.name));
                                    let mut other = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_default();
                                    let label = ui.label(RichText::new(tr("Another model")).color(t.text_faint).size(theme::CAPTION));
                                    let r = ui.add(egui::TextEdit::singleline(&mut other).desired_width(140.0)).labelled_by(label.id);
                                    let entered = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    if (widgets::link(ui, "Use").clicked() || entered) && !other.trim().is_empty() && other.trim() != current {
                                        chosen_model = Some((b.name.clone(), other.trim().to_string()));
                                        other.clear();
                                    }
                                    ui.data_mut(|d| d.insert_temp(id, other));
                                    if widgets::link(ui, "Cancel").clicked() {
                                        close_picker = true;
                                    }
                                }
                            }
                            if let Some(current) = effort {
                                ui.label(RichText::new(tr("Effort:")).color(t.text_faint).size(theme::CAPTION));
                                egui::ComboBox::from_id_salt(("effort", &b.name)).icon(widgets::combo_icon).selected_text(current.as_str()).show_ui(ui, |ui| {
                                    for e in craft_anything_ai::Effort::ALL {
                                        if ui.selectable_label(e == current, e.as_str()).clicked() && e != current {
                                            chosen_effort = Some((b.name.clone(), e));
                                        }
                                    }
                                });
                            }
                        });
                    }
                    if craft_anything_ai::model_options::supported(b) {
                        if let Some(draft) = app.model_tuning.get_mut(&b.name) {
                            if let Some(choices) = &draft.choices {
                                let mut changed = false;
                                ui.add_enabled_ui(!draft.saving, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(tr("Context window"));
                                        let label = draft
                                            .context
                                            .and_then(|value| choices.context.iter().find(|choice| choice.value == value))
                                            .map(|choice| tr(&choice.label))
                                            .unwrap_or_else(|| tr("Default"));
                                        egui::ComboBox::from_id_salt(("model-context", &b.name)).icon(widgets::combo_icon).selected_text(label).show_ui(
                                            ui,
                                            |ui| {
                                                changed |= ui.selectable_value(&mut draft.context, None, tr("Default")).changed();
                                                for choice in &choices.context {
                                                    changed |= ui.selectable_value(&mut draft.context, Some(choice.value), tr(&choice.label)).changed();
                                                }
                                            },
                                        );
                                    });
                                });
                                if changed {
                                    save_tuning = Some(draft.params());
                                }
                            } else {
                                model_options_loading(ui, &b.name);
                                if let Some(error) = &draft.error {
                                    ui.add(egui::Label::new(RichText::new(error).color(t.danger)).wrap());
                                    if icons::button(ui, "refresh-cw", 24.0, false, "Load model options").clicked() {
                                        tune_models.push(b.name.clone());
                                    }
                                } else {
                                    ui.spinner();
                                }
                            }
                        } else {
                            model_options_loading(ui, &b.name);
                            tune_models.push(b.name.clone());
                        }
                    }
                });
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_space(26.0);
                if app.state.backend.as_ref() == Some(&b.name) {
                    widgets::pill(ui, "In use", t.on_accent, t.accent);
                } else if widgets::secondary(ui, None, "Use").clicked() {
                    pick = Some(b.name.clone());
                }
                // Who builds: the chosen one says so (click to go back to automatic).
                if app.builder.as_ref() == Some(&b.name) {
                    if widgets::secondary(ui, Some("hammer"), "Builds").on_hover_text(tr("Builds use this one. Click to choose automatically again.")).clicked()
                    {
                        builds = Some(None);
                    }
                } else if b.enabled && widgets::secondary(ui, None, "Build with this").clicked() {
                    builds = Some(Some(b.name.clone()));
                }
                // Who takes a game's files out in Deep mode (an API backend, e.g. Kimi K3).
                if b.kind != BackendKind::AgentCli {
                    if app.extractor.as_ref() == Some(&b.name) {
                        if widgets::secondary(ui, Some("package"), "Extracts")
                            .on_hover_text(tr("In Deep mode this one takes a game's own files out after the study. Click to stop."))
                            .clicked()
                        {
                            extracts = Some(None);
                        }
                    } else if b.enabled && widgets::secondary(ui, None, "Extract with this").clicked() {
                        extracts = Some(Some(b.name.clone()));
                    }
                }
                // A new key for an API backend, checked with its provider before it's saved.
                let local_api = b.ssh_host.as_deref().is_none_or(|h| h.trim().is_empty());
                if b.kind != BackendKind::AgentCli
                    && local_api
                    && key_change.as_ref().is_none_or(|k| k.name != b.name)
                    && widgets::secondary(ui, None, "Change key").clicked()
                {
                    change_key_for = Some(b.name.clone());
                }
                // Off: kept with its key, but nothing uses it.
                let (label, tip) = if b.enabled {
                    ("Turn off", "Nothing uses it until it's on again; it keeps its key.")
                } else {
                    ("Turn on", "Builds and answers may use it again.")
                };
                if widgets::secondary(ui, None, label).on_hover_text(tr(tip)).clicked() {
                    toggle = Some((b.name.clone(), !b.enabled));
                }
                if confirming.as_ref() == Some(&b.name) {
                    if widgets::secondary(ui, Some("trash-2"), "Remove it and its key").clicked() {
                        remove = Some(b.name.clone());
                    }
                    if widgets::link(ui, "Keep it").clicked() {
                        ui.data_mut(|d| d.remove::<String>(confirm_id));
                    }
                } else if icons::button(ui, "trash-2", 26.0, false, &trf("Remove {name}", &[("name", &b.name)])).clicked() {
                    ui.data_mut(|d| d.insert_temp(confirm_id, b.name.clone()));
                }
            });
            if let Some(k) = key_change.as_mut().filter(|k| k.name == b.name) {
                let mut cancel = false;
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(26.0);
                    let field = egui::TextEdit::singleline(&mut k.key)
                        .password(true)
                        .hint_text(tr("API key (stored in your system keychain)"))
                        .desired_width(260.0)
                        .frame(widgets::text_field_frame_compact(&t));
                    widgets::named(ui.add_enabled(!k.saving, field), "API key");
                    let ready = !k.saving && !k.key.trim().is_empty();
                    if widgets::primary_icon(ui, None, "Save key", ready).clicked() && ready {
                        save_key = true;
                    }
                    if k.saving {
                        ui.add(crate::widgets::Spinner::new().color(t.accent));
                    } else if widgets::link(ui, "Cancel").clicked() {
                        cancel = true;
                    }
                });
                if let Some(error) = &k.error {
                    ui.add(egui::Label::new(RichText::new(error).color(t.danger)).wrap());
                }
                if cancel {
                    change_key_for = None;
                    key_change = None;
                }
            }
            if !b.enabled {
                ui.label(RichText::new(tr("Off")).color(t.text_faint).size(theme::CAPTION));
            }
        });
        ui.add_space(6.0);
    }
    if let Some((name, on)) = toggle {
        app.set_backend_enabled(&name, on);
    }
    if let Some(name) = choose_program {
        app.picking_program_for = Some(name);
        app.pick(Pick::AgentProgram);
    }
    if let Some(name) = usual_program {
        app.set_agent_program(&name, None);
    }
    app.folder_inputs = folder_drafts;
    // A key being saved stays open until its answer comes back.
    if let Some(name) = change_key_for.filter(|_| key_change.as_ref().is_none_or(|k| !k.saving)) {
        key_change = Some(crate::KeyChange { name, ..Default::default() });
    }
    app.key_change = key_change;
    if save_key {
        app.change_key();
    }
    if let Some((name, folder)) = save_folder {
        app.set_agent_folder(&name, Some(folder));
    }
    if let Some(name) = pick_model {
        app.pick_model(&name);
    }
    if let Some((name, program)) = remote_program {
        app.set_agent_program(&name, Some(program));
    }
    if let Some(name) = pick_agent_model {
        app.reload_agent_model(&name);
    }
    if let Some((name, model, thinking)) = chosen_agent_model {
        app.set_agent_model(&name, model, thinking);
    }
    if let Some((name, model)) = chosen_model {
        app.set_model(&name, &model);
    }
    if let Some((name, effort)) = chosen_effort {
        app.set_effort(&name, effort);
    }
    if close_picker {
        app.model_picker = None;
        app.agent_model_picker = None;
    }
    for name in tune_models {
        app.pick_model_options(&name);
    }
    if let Some(params) = save_tuning {
        app.set_model_options(params);
    }
    if let Some(name) = remove {
        ui.data_mut(|d| d.remove::<String>(confirm_id));
        app.remove_backend(&name);
    }
    if let Some(p) = pick {
        app.choose_backend(Some(p));
    }
    if let Some(b) = builds {
        app.set_role(craft_anything_ai::Role::Builder, b);
    }
    if let Some(e) = extracts {
        app.set_role(craft_anything_ai::Role::Extractor, e);
    }
    ui.add_space(8.0);
    let detected = app.detected.clone().unwrap_or_default();
    let mut tab = {
        let (providers, harnesses) = crate::detect_tabs::counts(&detected);
        crate::detect_tabs::resolve(app.state.detect_tab, providers, harnesses)
    };
    ui.horizontal(|ui| {
        widgets::caption(ui, "Found on this computer");
        ui.add_space(12.0);
        tab = crate::detect_tabs::bar(ui, &mut app.state.detect_tab, &detected);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.detecting {
                ui.add(crate::widgets::Spinner::new().color(t.accent));
            } else if widgets::secondary(ui, Some("radar"), "Detect").clicked() {
                app.detect_backends();
            }
        });
    });
    ui.add_space(4.0);
    if detected.is_empty() && !app.detecting {
        ui.label(
            RichText::new(tr("Nothing found yet. We look for API keys, local model servers (Ollama, LM Studio) and agent apps you already have. One on another port or computer can be added by its address below."))
                .color(t.text_faint),
        );
    }
    let shown = detected.iter().filter(|d| crate::detect_tabs::tab_of(d) == tab).count();
    if shown == 0 && !detected.is_empty() && !app.detecting {
        ui.label(RichText::new(tr("Nothing of this kind was found.")).color(t.text_faint));
    }
    for d in detected.into_iter().filter(|d| crate::detect_tabs::tab_of(d) == tab) {
        let configured = app.backends.iter().any(|b| b.name == d.suggestion.name);
        widgets::panel(&t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(&d.suggestion.name).font(theme::medium(theme::SMALL)));
                    ui.label(RichText::new(&d.reason).color(t.text_faint).size(theme::CAPTION));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if configured {
                        widgets::pill(ui, "Added", t.success, t.raised);
                        // Claude Code once more, to give it another account's folder.
                        if craft_anything_ai::folder_var(&d.suggestion).is_some()
                            && widgets::link(ui, "Add another").on_hover_text(tr("Add it again, to choose another account's folder for it.")).clicked()
                        {
                            let taken = |n: &String| app.backends.iter().any(|b| &b.name == n);
                            if let Some(name) = (2..100).map(|i| format!("{} {i}", d.suggestion.name)).find(|n| !taken(n)) {
                                app.add_backend(craft_anything_ai::BackendConfig { name, ..d.suggestion.clone() }, None);
                            }
                        }
                    } else if !d.needs_key && widgets::primary(ui, "Add").clicked() {
                        app.add_backend(d.suggestion.clone(), None);
                    }
                });
            });
            // A subscription key is worth pasting even when the environment already has one, so a
            // different key can be used without restarting the app.
            let pastable = d.needs_key || d.suggestion.base_url.as_deref().is_some_and(craft_anything_ai::openai::is_opencode_go);
            if pastable && !configured {
                ui.horizontal(|ui| {
                    let key = app.key_inputs.entry(d.suggestion.name.clone()).or_default();
                    ui.add(
                        egui::TextEdit::singleline(key)
                            .password(true)
                            .hint_text(tr("API key (stored in your system keychain)"))
                            .desired_width(260.0)
                            .frame(widgets::text_field_frame_compact(&Tokens::get(ui.ctx()))),
                    );
                    let k = key.trim().to_string();
                    if widgets::primary_icon(ui, None, "Add", !k.is_empty()).clicked() && !k.is_empty() {
                        // The key goes to the keychain; the app keeps no copy of it.
                        app.key_inputs.remove(&d.suggestion.name);
                        app.add_backend(d.suggestion.clone(), Some(k));
                    }
                });
            }
        });
        ui.add_space(6.0);
    }
    provider_form(app, ui);
    server_form(app, ui);
}

/// What a provider's billing pill says.
fn billing_pill(ui: &mut Ui, billing: craft_anything_ai::Billing) {
    use craft_anything_ai::Billing;
    let t = Tokens::get(ui.ctx());
    match billing {
        Billing::PayPerUse => widgets::pill(ui, "Pay per use", t.text_dim, t.raised),
        Billing::FreeKey => widgets::pill(ui, "Free key", t.success, t.raised),
        Billing::Subscription => widgets::pill(ui, "Monthly plan", t.text_dim, t.raised),
        Billing::OnYourComputer => widgets::pill(ui, "On your computer", t.success, t.raised),
        Billing::Unknown => widgets::pill(ui, "Billing unknown", t.text_faint, t.raised),
    };
}

/// An AI from a provider, added with its key: the Claude API, OpenAI, OpenRouter, Groq… (R5.8).
/// The provider is asked for its models with the key before anything is saved. The first run's
/// AI step shows it too.
pub(crate) fn provider_form(app: &mut CraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(14.0);
    widgets::caption(ui, "Add a provider");
    ui.label(
        RichText::new(tr("An AI service you have a key for. The key is checked with the provider before it's saved.")).color(t.text_faint).size(theme::CAPTION),
    );
    ui.add_space(4.0);
    // Taken out while the offers are borrowed, and put back after them.
    let mut form = std::mem::take(&mut app.provider_form);
    let offers = &app.offers;
    let chosen = offers.iter().find(|o| o.id == form.offer);
    let busy = form.adding.is_some();
    let mut add = false;
    ui.horizontal_wrapped(|ui| {
        let label = chosen.map_or_else(|| tr("Choose a provider"), |o| o.name.clone());
        let before = form.offer.clone();
        let combo = egui::ComboBox::from_id_salt("provider-offer").icon(widgets::combo_icon).width(240.0).selected_text(label).show_ui(ui, |ui| {
            for o in offers {
                ui.selectable_value(&mut form.offer, o.id.clone(), &o.name);
            }
        });
        widgets::named(combo.response, "Provider");
        if form.offer != before {
            // The name follows the provider until the person types their own.
            let old_default = offers.iter().find(|o| o.id == before).map(|o| o.default_name.as_str());
            if form.name.trim().is_empty() || Some(form.name.trim()) == old_default {
                form.name = offers.iter().find(|o| o.id == form.offer).map(|o| o.default_name.clone()).unwrap_or_default();
            }
            form.model.clear();
            form.error = None;
        }
        if let Some(o) = offers.iter().find(|o| o.id == form.offer) {
            billing_pill(ui, o.billing);
            if let Some(url) = o.key_url.as_deref()
                && widgets::link(ui, "Get a key").on_hover_text(url).clicked()
            {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            }
        }
    });
    if let Some(o) = offers.iter().find(|o| o.id == form.offer) {
        if let Some(note) = &o.note {
            ui.label(RichText::new(note).color(t.text_faint).size(theme::CAPTION));
        }
        const FRAME: f32 = 18.0;
        ui.horizontal(|ui| {
            let r = ui.add_enabled(
                !busy,
                egui::TextEdit::singleline(&mut form.name).hint_text(o.default_name.as_str()).desired_width(150.0).frame(widgets::text_field_frame_compact(&t)),
            );
            widgets::named(r, "AI name");
            let model = if form.model.is_empty() { trf("Model: {model}", &[("model", &o.default_model)]) } else { form.model.clone() };
            let combo = egui::ComboBox::from_id_salt("provider-model").icon(widgets::combo_icon).selected_text(model).show_ui(ui, |ui| {
                ui.selectable_value(&mut form.model, String::new(), tr("Default"));
                for m in &o.suggested_models {
                    ui.selectable_value(&mut form.model, m.clone(), m);
                }
            });
            widgets::named(combo.response, "Model");
        });
        ui.horizontal(|ui| {
            let w = (ui.available_width() - FRAME - 90.0).max(120.0);
            let hint = if o.needs_key { tr("API key (stored in your system keychain)") } else { tr("API key, if it needs one") };
            let r = ui.add_enabled(
                !busy,
                egui::TextEdit::singleline(&mut form.key).password(true).hint_text(hint).desired_width(w).frame(widgets::text_field_frame_compact(&t)),
            );
            widgets::named(r, "API key");
            let ready = !busy && (!o.needs_key || !form.key.trim().is_empty());
            if busy {
                ui.add(crate::widgets::Spinner::new().color(t.accent));
            } else {
                add = widgets::primary_icon(ui, Some("plus"), "Add", ready).clicked() && ready;
            }
        });
    }
    if let Some(error) = &form.error {
        ui.add(egui::Label::new(RichText::new(error).color(t.danger)).wrap());
    }
    app.provider_form = form;
    if add {
        app.add_provider();
    }
}

/// An AI that "Look for AI" can't find: a server on another computer or a hosted service, added by
/// its address (#54, #98). The server is asked for its models before it's added.
/// The first run's AI step shows it too.
pub(crate) fn server_form(app: &mut CraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(14.0);
    widgets::caption(ui, "Add an AI by its address");
    ui.label(RichText::new(tr("Ollama or LM Studio on another computer, or any service that speaks the OpenAI API.")).color(t.text_faint).size(theme::CAPTION));
    ui.add_space(4.0);
    let form = &mut app.server_form;
    // `width` is the text's; the compact frame adds 8 points a side and its border.
    const FRAME: f32 = 18.0;
    let field = |ui: &mut Ui, text: &mut String, hint: &str, width: f32, name: &str, secret: bool| {
        let r = ui.add(egui::TextEdit::singleline(text).password(secret).hint_text(tr(hint)).desired_width(width).frame(widgets::text_field_frame_compact(&t)));
        widgets::named(r, &tr(name));
    };
    ui.horizontal(|ui| {
        field(ui, &mut form.name, "name, e.g. Studio box", 150.0, "AI name", false);
        let w = ui.available_width() - FRAME;
        field(ui, &mut form.address, "address, e.g. http://192.168.1.20:11434/v1", w, "Server address", false);
    });
    let ready = !form.name.trim().is_empty() && !form.address.trim().is_empty();
    let mut add = false;
    ui.horizontal(|ui| {
        field(ui, &mut form.model, "model (optional)", 150.0, "Model", false);
        // Room for the Add button after it.
        let w = (ui.available_width() - FRAME - 90.0).max(120.0);
        field(ui, &mut form.key, "API key, if it needs one", w, "API key", true);
        add = widgets::primary_icon(ui, Some("plus"), "Add", ready).clicked() && ready;
    });
    if add {
        app.add_server();
    }
}

/// A number field; true when it changed.
fn number<T: egui::emath::Numeric>(ui: &mut Ui, value: &mut T, range: std::ops::RangeInclusive<T>, prefix: &str, suffix: &str) -> bool {
    ui.add(egui::DragValue::new(value).range(range).prefix(prefix).suffix(suffix)).changed()
}

fn builds(app: &mut CraftApp, ui: &mut Ui) {
    let mut changed = false;
    let b = &mut app.settings.build;
    row(ui, "Good enough", "How alike a rebuild must look to the original before refining stops.", |ui| {
        changed |= number(ui, &mut b.good_enough, 50.0..=100.0, "", " %");
    });
    row(ui, "Refining rounds", "At most this many rounds of fixes after the first build.", |ui| {
        changed |= number(ui, &mut b.max_rounds, 0..=20, "", "");
    });
    row(ui, "Website pages", "Other pages of a website studied and compared besides the one you gave.", |ui| {
        changed |= number(ui, &mut b.max_pages, 0..=craft_anything_ai::rebuild::kinds::website::MAX_ROUTES, "", "");
    });
    row(ui, "Match my pictures", "Apps and games: keep refining until they look this alike to your pictures. Off, the likeness is only reported.", |ui| {
        let mut on = b.picture_good_enough.is_some();
        if widgets::toggle(ui, &mut on, "Use as a goal").changed() {
            b.picture_good_enough = on.then_some(80.0);
            changed = true;
        }
        if let Some(v) = &mut b.picture_good_enough {
            changed |= number(ui, v, 30.0..=100.0, "", " %");
        }
    });
    row(ui, "Builds at once", "How many projects may build at the same time. More wait their turn.", |ui| {
        changed |= number(ui, &mut b.parallel, 1..=8, "", "");
    });
    ui.add_space(8.0);
    widgets::caption(ui, "Generated art (games)");
    let a = &mut app.settings.assets;
    row(
        ui,
        "Budget for a new project",
        "What a new game may spend on generated pictures and 3D models before you're asked. Each project keeps its own.",
        |ui| {
            changed |= number(ui, &mut a.budget_usd, 0.0..=10_000.0, "$", "");
        },
    );
    let table = craft_anything_engines::assets::Catalogue::built_in();
    row(ui, "Picture model", "What pictures and textures are made with.", |ui| {
        let current = table.image_model(a.image_model.as_deref()).or_else(|_| table.image_model(None)).map(|m| m.name.clone()).unwrap_or_default();
        egui::ComboBox::from_id_salt("image-model").icon(widgets::combo_icon).selected_text(current).show_ui(ui, |ui| {
            for m in table.images() {
                let price = crate::usage::asset_cost_words(m.cost(craft_anything_engines::assets::Shape::Square));
                let label = format!("{} · {} {}", m.name, price, tr("a picture"));
                let on = a.image_model.as_deref() == Some(m.id.as_str());
                if ui.selectable_label(on, label).on_hover_text(&m.licence).clicked() && !on {
                    a.image_model = Some(m.id.clone());
                    changed = true;
                }
            }
        });
    });
    if changed {
        app.settings_changed();
    }
}

/// Plugins: external MCP servers (`[[plugins.mcp]]`) and WASM components (`[[plugins.wasm]]`),
/// listed, turned on or off, added and removed.
fn plugins(app: &mut CraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.add(
        egui::Label::new(
            RichText::new(tr(
                "Plugins add tools Craft Anything offers as its own: MCP servers (a command or an address; read their tools after adding) or WebAssembly components (a .wasm file, which can't touch your files or the network).",
            ))
            .color(t.text_faint)
            .size(theme::CAPTION),
        )
        .wrap(),
    );
    ui.add_space(10.0);
    let plugins = app.settings.plugins.mcp.clone();
    let wasm = app.settings.plugins.wasm.clone();
    if plugins.is_empty() && wasm.is_empty() {
        ui.label(RichText::new(tr("No plugins yet.")).color(t.text_faint));
    }
    let mut changed = false;
    for (i, p) in plugins.iter().enumerate() {
        widgets::panel(&t).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    ui.label(RichText::new(p.display_name()).font(theme::medium(theme::SMALL)));
                    ui.label(RichText::new(p.place()).font(theme::mono(theme::CAPTION)).color(t.text_faint));
                    if let Some(problem) = p.problem() {
                        ui.label(RichText::new(problem).color(t.warning).size(theme::CAPTION));
                    }
                    if !p.consent {
                        ui.label(RichText::new(tr("Runs its tools without asking")).color(t.warning).size(theme::CAPTION));
                    }
                });
                if widgets::toggle(ui, &mut app.settings.plugins.mcp[i].enabled, "On").changed() {
                    changed = true;
                }
                if icons::button(ui, "refresh-cw", 26.0, false, "Read its tools").clicked() {
                    app.act("plugins_refresh", serde_json::json!({"id": p.id}), tr("Read the plugin's tools"));
                }
                if icons::button(ui, "trash-2", 26.0, false, "Remove").clicked() {
                    app.act("plugins_remove", serde_json::json!({"id": p.id}), tr("Remove a plugin"));
                }
            });
        });
        ui.add_space(6.0);
    }
    for (i, p) in wasm.iter().enumerate() {
        widgets::panel(&t).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    ui.label(RichText::new(p.display_name()).font(theme::medium(theme::SMALL)));
                    ui.label(RichText::new(p.path.display().to_string()).font(theme::mono(theme::CAPTION)).color(t.text_faint));
                    if let Some(problem) = p.problem() {
                        ui.label(RichText::new(problem).color(t.warning).size(theme::CAPTION));
                    }
                });
                if widgets::toggle(ui, &mut app.settings.plugins.wasm[i].enabled, "On").changed() {
                    changed = true;
                }
                if icons::button(ui, "trash-2", 26.0, false, "Remove").clicked() {
                    app.act("plugins_remove", serde_json::json!({"id": p.id}), tr("Remove a plugin"));
                }
            });
        });
        ui.add_space(6.0);
    }
    if changed {
        app.settings_changed();
    }
    if !plugins.is_empty() || !wasm.is_empty() {
        ui.label(RichText::new(tr("Turning a plugin on or off takes effect from the next start.")).color(t.text_faint).size(theme::CAPTION));
    }
    ui.add_space(14.0);
    widgets::caption(ui, "Add a plugin");
    ui.add_space(4.0);
    let form = &mut app.state.plugin_form;
    ui.horizontal(|ui| {
        let id =
            ui.add(egui::TextEdit::singleline(&mut form.id).hint_text(tr("id, e.g. docs")).desired_width(120.0).frame(widgets::text_field_frame_compact(&t)));
        widgets::named(id, "Plugin id");
        let place = ui.add(
            egui::TextEdit::singleline(&mut form.place)
                .hint_text(tr("command, https:// address or .wasm file"))
                .desired_width(200.0)
                .frame(widgets::text_field_frame_compact(&t)),
        );
        widgets::named(place, "Plugin command or address");
        let args = ui.add(
            egui::TextEdit::singleline(&mut form.args).hint_text(tr("arguments (optional)")).desired_width(160.0).frame(widgets::text_field_frame_compact(&t)),
        );
        widgets::named(args, "Plugin arguments");
    });
    ui.add_space(6.0);
    let params = app.state.plugin_form.params();
    if widgets::primary_icon(ui, Some("plus"), "Add", params.is_some()).clicked()
        && let Some(params) = params
    {
        app.act("plugins_add", params, tr("Add a plugin"));
        app.state.plugin_form = Default::default();
    }
    ui.label(RichText::new(tr("Plugins added here always ask before each call.")).color(t.text_faint).size(theme::CAPTION));
}

fn safety(app: &mut CraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    row(
        ui,
        "Ask before running anything",
        "Tools that run code from a target or change a phone always ask first, unless you allowed them for a project.",
        |ui| {
            if widgets::toggle(ui, &mut app.settings.ask_before_running, "Ask first").changed() {
                app.settings_changed();
            }
        },
    );
    if !app.settings.ask_before_running {
        ui.horizontal(|ui| {
            icons::show(ui, "triangle-alert", 15.0, t.warning);
            ui.label(RichText::new(tr("Tools may now run code from what you open without asking. Turn this back on unless you're sure.")).color(t.warning));
        });
    }
    row(
        ui,
        "Auto mode",
        "Builds keep going without stopping to ask you before an action. The builder still never deletes outside its project, changes system settings or publishes anything, and stays within each project's asset budget.",
        |ui| {
            if widgets::toggle(ui, &mut app.settings.auto_mode, "Keep going without asking").changed() {
                app.settings_changed();
            }
        },
    );
    row(
        ui,
        "The builder's questions",
        "When the builder needs a choice made: show it to you and wait, or let it decide and say what it chose. Each project can choose for itself under its prompt box.",
        |ui| {
            use craft_anything_engine::workspace::Questions;
            let sel = usize::from(app.settings.questions() == Questions::Decide);
            if let Some(i) = widgets::segmented(ui, &["Ask me", "Decide yourself"], sel, 30.0) {
                app.settings.questions = Some(if i == 0 { Questions::Ask } else { Questions::Decide });
                app.settings_changed();
            }
        },
    );
    row(
        ui,
        "Study apps by running them",
        "When you rebuild an app or game, start it once to picture its window and compare the rebuild with it. It runs that program.",
        |ui| {
            if widgets::toggle(ui, &mut app.settings.runtime_study, "Run to study").changed() {
                app.settings_changed();
            }
        },
    );
    if let Some(p) = app.view.project.clone() {
        ui.add_space(8.0);
        widgets::caption(ui, &format!("{} {}", tr("Allowed in"), p));
        if app.view.project_allows.is_empty() {
            ui.label(RichText::new(tr("Nothing allowed yet.")).color(t.text_faint));
        }
        for tool in app.view.project_allows.clone() {
            ui.label(RichText::new(tool).font(theme::mono(12.0)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Done, builders::AgentModelPicker, demo, jobs::Finished};
    use craft_anything_ai::{BackendConfig, BackendKind};

    #[test]
    fn identical_model_names_are_shown_once_in_the_shared_picker() {
        use craft_anything_ai::{AgentModel, BackendEntry, ModelContext};
        use egui_kittest::{Harness, kittest::Queryable};
        let context = ModelContext {
            backend: BackendEntry {
                config: BackendConfig {
                    name: "Test agent".into(),
                    kind: BackendKind::AgentCli,
                    model: "copilot".into(),
                    base_url: None,
                    ssh_host: None,
                    enabled: true,
                },
                options: Default::default(),
            },
            directory: None,
        };
        let mut picker = AgentModelPicker {
            request: 1,
            context,
            model: None,
            thinking: None,
            result: Some(Ok(vec![
                AgentModel { id: "auto".into(), name: "auto".into(), thinking: Vec::new() },
                AgentModel { id: "provider/model".into(), name: "Friendly name".into(), thinking: Vec::new() },
            ])),
        };
        let mut harness = Harness::builder().build_ui(move |ui| {
            theme::install_fonts(ui.ctx());
            crate::builders::agent_model_picker(ui, &mut picker);
        });
        harness.get_by_label("Model").click();
        harness.run_steps(3);
        assert!(harness.query_by_label("auto").is_some());
        assert!(harness.query_by_label("auto · auto").is_none());
        assert!(harness.query_by_label("Friendly name · provider/model").is_some());
    }

    #[test]
    fn shared_picker_warnings_name_the_configured_agent() {
        use craft_anything_ai::{BackendEntry, ModelContext};
        use egui_kittest::{Harness, kittest::Queryable};
        for (agent, name) in [("copilot", "GitHub Copilot"), ("opencode", "OpenCode")] {
            let context = ModelContext {
                backend: BackendEntry {
                    config: BackendConfig {
                        name: "Test".into(),
                        kind: BackendKind::AgentCli,
                        model: agent.into(),
                        base_url: None,
                        ssh_host: None,
                        enabled: true,
                    },
                    options: Default::default(),
                },
                directory: None,
            };
            let mut picker = AgentModelPicker { request: 1, context, model: Some("missing-model".into()), thinking: None, result: Some(Ok(Vec::new())) };
            let harness = Harness::builder().build_ui(move |ui| {
                theme::install_fonts(ui.ctx());
                crate::builders::agent_model_picker(ui, &mut picker);
            });
            harness.get_by_label(format!("No chat models are visible in {name}. Check its setup, then reload.").as_str());
            harness.get_by_label(format!("The saved model is no longer visible. Choose another model or the {name} default.").as_str());
            if agent == "copilot" {
                assert!(harness.query_by_label("The saved model is no longer visible. Choose another model or the OpenCode default.").is_none());
            }
        }
    }

    /// Busy frames retain matching catalogue results, ignore older requests and defer context invalidation until readable.
    #[test]
    fn a_busy_session_keeps_the_picker_and_its_matching_result() {
        let mut app = demo::app(demo::engine(craft_anything_engine::Mode::Experienced, true));
        app.hub
            .add_backend(
                BackendConfig { name: "OpenCode".into(), kind: BackendKind::AgentCli, model: "opencode".into(), base_url: None, ssh_host: None, enabled: true },
                None,
            )
            .unwrap();
        app.backends = app.hub.backends();
        app.backend_options = app.hub.options();
        let context = app.hub.model_context("OpenCode").unwrap();
        app.agent_model_picker = Some(AgentModelPicker { request: 2, context: context.clone(), result: None, model: None, thinking: None });
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let draw = |app: &mut CraftApp| {
            let mut output = ctx.run_ui(Default::default(), |ui| ai(app, ui));
            output.textures_delta.clear();
        };
        let engine = app.engine.clone();
        let session = engine.session();
        app.handle_done(Finished { job: 0, result: Ok(Done::AgentModelChoices { request: 1, context: context.clone(), result: Ok(Vec::new()) }) });
        assert!(app.agent_model_picker.as_ref().unwrap().result.is_none(), "an older reload must still be ignored");
        app.handle_done(Finished { job: 0, result: Ok(Done::AgentModelChoices { request: 2, context, result: Ok(Vec::new()) }) });
        draw(&mut app);
        assert!(app.agent_model_picker.as_ref().unwrap().result.as_ref().unwrap().is_ok(), "a busy frame must keep the completed catalogue");
        drop(session);
        draw(&mut app);
        assert!(app.agent_model_picker.is_some(), "unlocking an unchanged session keeps the draft");
        app.hub.set_enabled("OpenCode", false).unwrap();
        draw(&mut app);
        assert!(app.agent_model_picker.is_none(), "a confirmed settings change still invalidates the picker");
    }

    /// An AI that finishes connecting is chosen, unless another choice (here "No AI for now") was
    /// made while it connected.
    #[test]
    fn a_late_add_does_not_undo_a_choice_made_meanwhile() {
        let mut app = demo::app(demo::engine(craft_anything_engine::Mode::Beginner, false));
        let added = |choices| Ok(Done::BackendAdded { name: "Home server".into(), result: Ok(()), choices });
        let started = app.backend_choices;
        app.choose_backend(None);
        app.handle_done(Finished { job: 0, result: added(started) });
        assert_eq!(app.state.backend, None, "the late add must not override No AI for now");
        app.handle_done(Finished { job: 0, result: added(app.backend_choices) });
        assert_eq!(app.state.backend.as_deref(), Some("Home server"), "with no choice meanwhile, the added AI is chosen");
        // Two adds at once: the first one's selection isn't a choice, so the second is selected too.
        let started = app.backend_choices;
        let other = |choices| Ok(Done::BackendAdded { name: "Laptop".into(), result: Ok(()), choices });
        app.handle_done(Finished { job: 0, result: added(started) });
        app.handle_done(Finished { job: 0, result: other(started) });
        assert_eq!(app.state.backend.as_deref(), Some("Laptop"), "the add that finished last is selected");
    }

    /// A chat started while an AI connects is a choice too: the add doesn't switch AI under it.
    #[test]
    fn a_late_add_does_not_stop_a_chat_started_meanwhile() {
        let mut app = demo::app(demo::engine(craft_anything_engine::Mode::Experienced, true));
        // A closed local port: the chat starts, and its request fails without leaving this computer.
        let config = BackendConfig {
            name: "Box".into(),
            kind: BackendKind::OpenaiCompatible,
            model: "m".into(),
            base_url: Some("http://127.0.0.1:9/v1".into()),
            ssh_host: None,
            enabled: true,
        };
        app.hub.add_backend(config, None).unwrap();
        app.state.backend = Some("Box".into());
        let started = app.backend_choices;
        app.send("Hi");
        assert!(app.conversation.is_some(), "the chat started");
        app.handle_done(Finished { job: 0, result: Ok(Done::BackendAdded { name: "Laptop".into(), result: Ok(()), choices: started }) });
        assert_eq!(app.state.backend.as_deref(), Some("Box"), "the chat's AI stays chosen");
        assert!(app.conversation.is_some(), "the chat keeps going");
        // A follow-up in a chat that started before the add counts the same way.
        let started = app.backend_choices;
        app.send("And then?");
        app.handle_done(Finished { job: 0, result: Ok(Done::BackendAdded { name: "Laptop".into(), result: Ok(()), choices: started }) });
        assert_eq!(app.state.backend.as_deref(), Some("Box"), "a follow-up keeps its AI");
        assert!(app.conversation.is_some(), "the follow-up's answer keeps going");
    }
}

#[cfg(test)]
mod model_options_tests {
    use super::*;
    use craft_anything_ai::config::{CliContext, CliReasoningEffort};
    use craft_anything_ai::model_options::{ModelOption, ModelTuning};

    fn app_with_backend() -> CraftApp {
        let mut app = crate::demo::app(crate::demo::engine(craft_anything_engine::Mode::Beginner, false));
        app.engine.session().settings.ai = Default::default();
        app.backends = vec![craft_anything_ai::BackendConfig {
            name: "Copilot test".into(),
            kind: BackendKind::AgentCli,
            model: "copilot".into(),
            base_url: None,
            ssh_host: None,
            enabled: true,
        }];
        app
    }

    fn render(app: &mut CraftApp) -> String {
        let context = egui::Context::default();
        crate::theme::install_fonts(&context);
        let mut output = context.run_ui(egui::RawInput::default(), |ui| ai(app, ui));
        output.textures_delta.clear();
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) => Some(text.galley.text().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn model_controls_use_shared_thinking_and_separate_context_without_loading_a_cli() {
        let mut app = app_with_backend();
        let mut draft = crate::ModelTuningDraft {
            backend: "Copilot test".into(),
            cli_model: Some("test-model".into()),
            choices: None,
            context: Some(CliContext::Long),
            saving: false,
            error: None,
        };
        draft.set_choices(ModelTuning {
            model: "test-model".into(),
            context: vec![ModelOption { value: CliContext::Long, label: "1M".into(), description: None }],
            reasoning: vec![ModelOption { value: CliReasoningEffort::High, label: "high".into(), description: None }],
        });
        app.model_tuning.insert(draft.backend.clone(), draft);
        let text = render(&mut app);
        assert!(text.contains("Context window") && text.contains("Thinking depth") && text.contains("1M") && !text.contains("Reasoning"), "{text}");
        assert_eq!(app.model_tuning["Copilot test"].params().context, Some(CliContext::Long));
    }

    #[test]
    fn model_controls_are_always_visible_and_discovery_is_not_requeued_each_frame() {
        let mut app = app_with_backend();
        let text = render(&mut app);
        assert!(text.contains("Context window") && text.contains("Thinking depth"), "{text}");
        let jobs = app.jobs.list.len();
        assert!(app.model_tuning.contains_key("Copilot test"));
        render(&mut app);
        assert_eq!(app.jobs.list.len(), jobs);
        app.model_tuning.get_mut("Copilot test").unwrap().error = Some("Discovery unavailable".into());
        let text = render(&mut app);
        assert!(text.contains("Context window") && text.contains("Thinking depth") && text.contains("Discovery unavailable"), "{text}");
        assert_eq!(app.jobs.list.len(), jobs, "failed discovery waits for an explicit retry");
    }

    #[test]
    fn model_option_changes_save_without_closing_the_controls() {
        let mut app = app_with_backend();
        app.hub.add_backend(app.backends[0].clone(), None).unwrap();
        app.backend_options = app.hub.options();
        let state = crate::ModelTuningDraft {
            backend: "Copilot test".into(),
            cli_model: None,
            choices: None,
            context: Some(CliContext::Long),
            saving: false,
            error: None,
        };
        let params = state.params();
        app.model_tuning.insert(state.backend.clone(), state);
        app.set_model_options(params);
        assert!(app.model_tuning["Copilot test"].saving);
        let started = std::time::Instant::now();
        while app.model_tuning["Copilot test"].saving && started.elapsed() < std::time::Duration::from_secs(3) {
            for finished in app.jobs.poll() {
                app.handle_done(finished);
            }
            std::thread::park_timeout(std::time::Duration::from_millis(5));
        }
        assert!(!app.model_tuning["Copilot test"].saving);
        assert_eq!(app.hub.options()["Copilot test"].cli_context, Some(CliContext::Long));
        assert_eq!(app.backend_options["Copilot test"].cli_thinking, None);
    }

    #[test]
    fn choosing_a_model_automatically_repopulates_its_options() {
        let mut app = app_with_backend();
        app.hub
            .add_backend_with(
                craft_anything_ai::BackendEntry {
                    config: app.backends[0].clone(),
                    options: craft_anything_ai::BackendOptions {
                        cli_model: Some("old-model".into()),
                        command: Some("craft-anything-no-such-test-agent".into()),
                        ..Default::default()
                    },
                },
                None,
            )
            .unwrap();
        app.backend_options = app.hub.options();
        app.model_tuning.insert(
            "Copilot test".into(),
            crate::ModelTuningDraft {
                backend: "Copilot test".into(),
                cli_model: Some("old-model".into()),
                choices: None,
                context: None,
                saving: false,
                error: Some("Old choices".into()),
            },
        );
        let mut options = app.backend_options["Copilot test"].clone();
        options.cli_model = Some("new-model".into());
        app.hub.add_backend_with(craft_anything_ai::BackendEntry { config: app.backends[0].clone(), options }, None).unwrap();
        let result = crate::backends(&app.hub);
        app.handle_done(crate::jobs::Finished { job: 0, result: Ok(result) });
        assert_eq!(app.backend_options["Copilot test"].cli_model.as_deref(), Some("new-model"));
        assert!(!app.model_tuning.contains_key("Copilot test"));
        let text = render(&mut app);
        assert!(text.contains("new-model") && text.contains("Context window") && text.contains("Thinking depth"), "{text}");
        assert_eq!(app.model_tuning["Copilot test"].cli_model.as_deref(), Some("new-model"));
        assert_eq!(app.model_tuning["Copilot test"].error, None);
    }
}
