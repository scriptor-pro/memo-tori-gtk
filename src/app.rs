use std::cell::RefCell;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use gtk::gdk;
use gtk::gio;
use gtk::glib::variant::ToVariant;
use gtk::glib::Propagation;
use gtk::prelude::*;
use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Label, ListBox,
    ListBoxRow, Orientation, Paned, Popover, PopoverMenuBar, ScrolledWindow, SearchEntry, Stack,
    StackSwitcher, TextView, WrapMode,
};
use notify_rust::Notification;
use rusqlite::Connection;

use crate::config::AppConfig;
use crate::db;

fn clear_listbox(list_box: &ListBox) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }
}

fn note_title(content: &str) -> String {
    content
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().chars().take(60).collect::<String>())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "(empty note)".to_string())
}

fn parse_tags(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|part| part.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect()
}

fn current_tag_fragment(input: &str) -> String {
    input
        .rsplit(',')
        .next()
        .unwrap_or_default()
        .trim()
        .to_lowercase()
}

fn apply_tag_completion(input: &str, completion: &str) -> String {
    let mut parts: Vec<String> = input
        .split(',')
        .map(|part| part.trim().to_string())
        .collect();

    if parts.is_empty() {
        return format!("{}, ", completion);
    }

    parts.pop();
    parts.push(completion.to_string());

    let mut rebuilt = parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    rebuilt.push_str(", ");
    rebuilt
}

fn attach_tag_autocomplete(entry: &Entry, conn: Rc<RefCell<Connection>>) {
    let popover = Popover::new();
    popover.set_has_arrow(false);
    popover.set_autohide(true);
    popover.set_parent(entry);

    let suggestions = ListBox::new();
    suggestions.set_activate_on_single_click(true);
    popover.set_child(Some(&suggestions));

    suggestions.connect_row_activated({
        let entry = entry.clone();
        let popover = popover.clone();
        move |_, row| {
            let Some(child) = row.child() else {
                return;
            };

            let Ok(label) = child.downcast::<Label>() else {
                return;
            };

            let completion = label.text().to_string();
            let updated = apply_tag_completion(&entry.text(), &completion);
            entry.set_text(&updated);
            entry.set_position(-1);
            popover.popdown();
        }
    });

    entry.connect_changed({
        let entry = entry.clone();
        let conn = Rc::clone(&conn);
        let suggestions = suggestions.clone();
        let popover = popover.clone();
        move |_| {
            let fragment = current_tag_fragment(&entry.text());
            if fragment.is_empty() {
                popover.popdown();
                return;
            }

            let tags = db::list_tags_prefix(&conn.borrow(), &fragment, 8).unwrap_or_default();
            let tags: Vec<String> = tags.into_iter().filter(|tag| tag != &fragment).collect();

            if tags.is_empty() {
                popover.popdown();
                return;
            }

            clear_listbox(&suggestions);
            for tag in tags {
                let row = ListBoxRow::new();
                let label = Label::new(Some(&tag));
                label.set_halign(Align::Start);
                label.set_xalign(0.0);
                label.set_margin_top(6);
                label.set_margin_bottom(6);
                label.set_margin_start(10);
                label.set_margin_end(10);
                row.set_child(Some(&label));
                suggestions.append(&row);
            }

            popover.popup();
        }
    });
}

fn random_hint(hints: &[String]) -> String {
    if hints.is_empty() {
        return "L'idee que je viens d'avoir :".to_string();
    }

    if hints.len() == 1 {
        return hints[0].clone();
    }

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as usize)
        .unwrap_or(0);

    hints[seed % hints.len()].clone()
}

fn icon_label_button(icon_name: &str, label_text: &str) -> Button {
    let button = Button::new();
    let content = GtkBox::new(Orientation::Horizontal, 6);

    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_icon_size(gtk::IconSize::Normal);
    let label = Label::new(Some(label_text));

    content.append(&icon);
    content.append(&label);
    button.set_child(Some(&content));
    button
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        "
window {
  background: linear-gradient(180deg, #f4f0e6 0%, #ece7dc 100%);
}

* {
  font-family: \"Inter\", \"Noto Sans\", \"DejaVu Sans\", sans-serif;
}

.menu-bar {
  background: #1f3a45;
  color: #f8fafc;
  padding: 6px;
  border-radius: 8px;
}

.switcher-wrap {
  background: #d8dfdc;
  padding: 6px;
  border-radius: 10px;
}

.capture-panel {
  background: #EDF5F3;
  border: 1px solid #b69f70;
  border-radius: 12px;
  padding: 12px;
}

.library-panel {
  background: #EDF5F3;
  border: 1px solid #b69f70;
  border-radius: 12px;
  padding: 12px;
}

.reader {
  background: #ffffff;
  color: #172127;
}

.section-title {
  color: #172127;
  font-weight: 700;
}

.status-label {
  color: #172127;
  font-weight: 600;
}

.placeholder-hint {
  color: #496067;
  font-style: italic;
}

.tag-chip {
  color: #1f3a45;
  background: #d9ecef;
  border-radius: 8px;
  padding: 4px 8px;
}

.capture-panel textview,
.library-panel entry,
.library-panel list,
.library-panel textview {
  background: #ffffff;
  color: #172127;
  border-radius: 8px;
}

button:focus-visible,
entry:focus-visible,
textview:focus-visible,
list:focus-visible {
  outline: 3px solid #0b6ea8;
  outline-offset: 2px;
}
",
    );

    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub fn run(config: AppConfig, connection: Connection) -> Result<()> {
    let quit_on_close = config.quit_on_close;
    let capture_hints = if config.capture_hints.is_empty() {
        vec!["L'idee que je viens d'avoir :".to_string()]
    } else {
        config.capture_hints.clone()
    };

    let app = Application::builder()
        .application_id("io.github.memo_tori.gtk")
        .build();

    let conn = Rc::new(RefCell::new(connection));
    let capture_hints = Rc::new(capture_hints);

    app.connect_activate(move |app| {
        gtk::Window::set_default_icon_name("memo-tori");
        install_css();

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Memo-Tori")
            .default_width(960)
            .default_height(680)
            .build();
        window.set_icon_name(Some("memo-tori"));

        let root = GtkBox::new(Orientation::Vertical, 8);
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        root.set_margin_start(12);
        root.set_margin_end(12);

        let stack = Stack::new();
        stack.set_vexpand(true);
        stack.set_hexpand(true);

        let stack_switcher = StackSwitcher::new();
        stack_switcher.set_stack(Some(&stack));
        stack_switcher.set_halign(Align::Center);

        let switcher_wrap = GtkBox::new(Orientation::Horizontal, 8);
        switcher_wrap.add_css_class("switcher-wrap");
        switcher_wrap.append(&stack_switcher);

        let app_menu = gio::Menu::new();

        let item_capture = gio::MenuItem::new(Some("Capture"), Some("app.show_capture"));
        item_capture.set_attribute_value("icon", Some(&"document-edit-symbolic".to_variant()));
        app_menu.append_item(&item_capture);

        let item_notes = gio::MenuItem::new(Some("Notes"), Some("app.show_notes"));
        item_notes.set_attribute_value("icon", Some(&"view-list-symbolic".to_variant()));
        app_menu.append_item(&item_notes);

        let item_quit = gio::MenuItem::new(Some("Quitter"), Some("app.quit"));
        item_quit.set_attribute_value("icon", Some(&"application-exit-symbolic".to_variant()));
        app_menu.append_item(&item_quit);

        let menu_root = gio::Menu::new();
        menu_root.append_submenu(Some("Memo-Tori"), &app_menu);

        let menu_bar = PopoverMenuBar::from_model(Some(&menu_root));
        menu_bar.add_css_class("menu-bar");

        let action_show_capture = gio::SimpleAction::new("show_capture", None);
        action_show_capture.connect_activate({
            let stack = stack.clone();
            move |_, _| stack.set_visible_child_name("capture")
        });
        app.add_action(&action_show_capture);

        let action_show_notes = gio::SimpleAction::new("show_notes", None);
        action_show_notes.connect_activate({
            let stack = stack.clone();
            move |_, _| stack.set_visible_child_name("notes")
        });
        app.add_action(&action_show_notes);

        let action_quit = gio::SimpleAction::new("quit", None);
        action_quit.connect_activate({
            let app = app.clone();
            move |_, _| app.quit()
        });
        app.add_action(&action_quit);
        app.set_accels_for_action("app.show_capture", &["<Primary>1"]);
        app.set_accels_for_action("app.show_notes", &["<Primary>2"]);
        app.set_accels_for_action("app.quit", &["<Primary>q"]);

        let capture_panel = GtkBox::new(Orientation::Vertical, 8);
        capture_panel.add_css_class("capture-panel");

        let capture_label = Label::new(Some("Capture d'idee rapide"));
        capture_label.set_halign(Align::Start);
        capture_label.add_css_class("section-title");

        let capture_scrolled = ScrolledWindow::new();
        capture_scrolled.set_vexpand(false);
        capture_scrolled.set_min_content_height(220);

        let text_view = TextView::new();
        text_view.set_wrap_mode(WrapMode::WordChar);
        text_view.set_vexpand(false);
        text_view.grab_focus();
        text_view.set_tooltip_text(Some(
            "Saisir une idee. Enter pour sauvegarder, Shift+Enter pour nouvelle ligne.",
        ));
        capture_scrolled.set_child(Some(&text_view));

        let capture_overlay = gtk::Overlay::new();
        capture_overlay.set_child(Some(&capture_scrolled));

        let placeholder_label = Label::new(Some(&random_hint(capture_hints.as_ref())));
        placeholder_label.add_css_class("placeholder-hint");
        placeholder_label.set_halign(Align::Start);
        placeholder_label.set_valign(Align::Start);
        placeholder_label.set_margin_top(10);
        placeholder_label.set_margin_start(10);
        placeholder_label.set_xalign(0.0);
        placeholder_label.set_can_target(false);
        capture_overlay.add_overlay(&placeholder_label);

        let capture_tags = Entry::new();
        capture_tags.set_placeholder_text(Some("Tags capture (ex: perso, urgent)"));
        capture_tags.set_tooltip_text(Some("Liste de tags separes par des virgules"));
        attach_tag_autocomplete(&capture_tags, Rc::clone(&conn));

        let actions = GtkBox::new(Orientation::Horizontal, 8);
        actions.set_halign(Align::End);

        let save_btn = icon_label_button("document-save-symbolic", "Save");
        let cancel_btn = icon_label_button("edit-clear-symbolic", "Cancel");
        save_btn.set_tooltip_text(Some("Sauvegarder la note"));
        cancel_btn.set_tooltip_text(Some("Effacer la saisie"));

        actions.append(&cancel_btn);
        actions.append(&save_btn);

        capture_panel.append(&capture_label);
        capture_panel.append(&capture_overlay);
        capture_panel.append(&capture_tags);
        capture_panel.append(&actions);

        let library_panel = GtkBox::new(Orientation::Vertical, 8);
        library_panel.add_css_class("library-panel");

        let search_row = GtkBox::new(Orientation::Horizontal, 8);
        let search_entry = SearchEntry::new();
        search_entry.set_hexpand(true);
        search_entry.set_placeholder_text(Some("Search notes (FTS5)"));
        search_entry.set_tooltip_text(Some("Recherche plein texte dans les notes"));
        let filter_tags_entry = Entry::new();
        filter_tags_entry.set_hexpand(true);
        filter_tags_entry.set_placeholder_text(Some("Filtre tags (ex: projet, idee)"));
        filter_tags_entry.set_tooltip_text(Some("Affiche les notes qui contiennent tous ces tags"));
        attach_tag_autocomplete(&filter_tags_entry, Rc::clone(&conn));
        let status_label = Label::new(Some("0 notes"));
        status_label.set_halign(Align::End);
        status_label.add_css_class("status-label");
        search_row.append(&search_entry);
        search_row.append(&filter_tags_entry);
        search_row.append(&status_label);

        let edit_tags_row = GtkBox::new(Orientation::Horizontal, 8);
        let selected_tags_entry = Entry::new();
        selected_tags_entry.set_hexpand(true);
        selected_tags_entry.set_placeholder_text(Some("Tags de la note selectionnee"));
        attach_tag_autocomplete(&selected_tags_entry, Rc::clone(&conn));
        let apply_tags_btn = icon_label_button("emblem-ok-symbolic", "Apply tags");
        apply_tags_btn.set_tooltip_text(Some("Appliquer les tags a la note selectionnee"));
        let save_note_btn = icon_label_button("document-save-symbolic", "Save note");
        save_note_btn.set_tooltip_text(Some("Sauvegarder les modifications de la note"));
        edit_tags_row.append(&selected_tags_entry);
        edit_tags_row.append(&apply_tags_btn);
        edit_tags_row.append(&save_note_btn);

        let selected_tags_label = Label::new(Some("Tags: -"));
        selected_tags_label.set_halign(Align::Start);
        selected_tags_label.add_css_class("tag-chip");

        let paned = Paned::new(Orientation::Horizontal);
        paned.set_wide_handle(true);
        paned.set_resize_start_child(true);
        paned.set_shrink_start_child(false);

        let list_box = ListBox::new();
        list_box.set_selection_mode(gtk::SelectionMode::Single);

        let list_scrolled = ScrolledWindow::new();
        list_scrolled.set_hexpand(true);
        list_scrolled.set_vexpand(true);
        list_scrolled.set_min_content_width(280);
        list_scrolled.set_child(Some(&list_box));

        let reader = TextView::new();
        reader.add_css_class("reader");
        reader.set_editable(true);
        reader.set_cursor_visible(true);
        reader.set_wrap_mode(WrapMode::WordChar);

        let reader_scrolled = ScrolledWindow::new();
        reader_scrolled.set_hexpand(true);
        reader_scrolled.set_vexpand(true);
        reader_scrolled.set_min_content_width(420);
        reader_scrolled.set_child(Some(&reader));

        paned.set_start_child(Some(&list_scrolled));
        paned.set_end_child(Some(&reader_scrolled));
        paned.set_position(320);

        library_panel.append(&search_row);
        library_panel.append(&edit_tags_row);
        library_panel.append(&selected_tags_label);
        library_panel.append(&paned);

        stack.add_titled(&capture_panel, Some("capture"), "Capture");
        stack.add_titled(&library_panel, Some("notes"), "Notes");
        stack.set_visible_child_name("capture");

        root.append(&menu_bar);
        root.append(&switcher_wrap);
        root.append(&stack);
        window.set_child(Some(&root));

        let notes_state = Rc::new(RefCell::new(Vec::<db::NoteListItem>::new()));

        let refresh_notes: Rc<dyn Fn()> = {
            let conn = Rc::clone(&conn);
            let search_entry = search_entry.clone();
            let filter_tags_entry = filter_tags_entry.clone();
            let list_box = list_box.clone();
            let reader = reader.clone();
            let status_label = status_label.clone();
            let notes_state = Rc::clone(&notes_state);
            let selected_tags_label = selected_tags_label.clone();
            let selected_tags_entry = selected_tags_entry.clone();

            Rc::new(move || {
                let query = search_entry.text().to_string();
                let filter_tags = parse_tags(&filter_tags_entry.text());

                match db::search_notes(&conn.borrow(), &query, &filter_tags, 200) {
                    Ok(notes) => {
                        clear_listbox(&list_box);
                        *notes_state.borrow_mut() = notes.clone();

                        for item in &notes {
                            let row = ListBoxRow::new();
                            let container = GtkBox::new(Orientation::Vertical, 2);
                            container.set_margin_top(8);
                            container.set_margin_bottom(8);
                            container.set_margin_start(8);
                            container.set_margin_end(8);

                            let title = Label::new(Some(&note_title(&item.preview)));
                            title.set_halign(Align::Start);
                            title.set_xalign(0.0);
                            title.add_css_class("section-title");

                            container.append(&title);
                            row.set_child(Some(&container));
                            list_box.append(&row);
                        }

                        status_label.set_text(&format!("{} notes", notes.len()));

                        if let Some(row) = list_box.row_at_index(0) {
                            list_box.select_row(Some(&row));
                        } else {
                            reader.buffer().set_text("No notes yet.");
                            selected_tags_label.set_text("Tags: -");
                            selected_tags_entry.set_text("");
                        }
                    }
                    Err(err) => {
                        status_label.set_text("Search error");
                        reader
                            .buffer()
                            .set_text(&format!("Search failed:\n{}", err));
                    }
                }
            })
        };

        let on_save = {
            let text_view = text_view.clone();
            let capture_tags = capture_tags.clone();
            let conn = Rc::clone(&conn);
            let refresh_notes = Rc::clone(&refresh_notes);
            move || {
                let buffer = text_view.buffer();
                let start = buffer.start_iter();
                let end = buffer.end_iter();
                let content = buffer.text(&start, &end, true);
                let trimmed = content.trim();

                if trimmed.is_empty() {
                    return;
                }

                let tags = parse_tags(&capture_tags.text());

                if db::insert_note(&mut conn.borrow_mut(), trimmed, &tags).is_ok() {
                    buffer.set_text("");
                    capture_tags.set_text("");
                    let _ = Notification::new()
                        .summary("Memo-Tori")
                        .body("Note saved")
                        .show();
                    refresh_notes.as_ref()();
                }
            }
        };

        text_view.buffer().connect_changed({
            let text_view = text_view.clone();
            let placeholder_label = placeholder_label.clone();
            let capture_hints = Rc::clone(&capture_hints);
            let was_empty = Rc::new(RefCell::new(true));

            move |_| {
                let buffer = text_view.buffer();
                let start = buffer.start_iter();
                let end = buffer.end_iter();
                let is_empty = buffer.text(&start, &end, true).trim().is_empty();

                if is_empty {
                    if !*was_empty.borrow() {
                        placeholder_label.set_text(&random_hint(capture_hints.as_ref()));
                    }
                    placeholder_label.set_visible(true);
                } else {
                    placeholder_label.set_visible(false);
                }

                *was_empty.borrow_mut() = is_empty;
            }
        });

        save_btn.connect_clicked({
            let on_save = on_save.clone();
            move |_| on_save()
        });

        cancel_btn.connect_clicked({
            let text_view = text_view.clone();
            move |_| {
                text_view.buffer().set_text("");
            }
        });

        list_box.connect_row_selected({
            let conn = Rc::clone(&conn);
            let reader = reader.clone();
            let notes_state = Rc::clone(&notes_state);
            let selected_tags_label = selected_tags_label.clone();
            let selected_tags_entry = selected_tags_entry.clone();
            move |_, row| {
                let Some(row) = row else {
                    reader.buffer().set_text("No note selected.");
                    selected_tags_label.set_text("Tags: -");
                    selected_tags_entry.set_text("");
                    return;
                };

                let index = row.index();
                if index < 0 {
                    reader.buffer().set_text("No note selected.");
                    selected_tags_label.set_text("Tags: -");
                    selected_tags_entry.set_text("");
                    return;
                }

                let note_id = notes_state
                    .borrow()
                    .get(index as usize)
                    .map(|note| note.id.clone());

                let Some(note_id) = note_id else {
                    reader.buffer().set_text("No note selected.");
                    selected_tags_label.set_text("Tags: -");
                    selected_tags_entry.set_text("");
                    return;
                };

                match db::get_note_content(&conn.borrow(), &note_id) {
                    Ok(Some(content)) => reader.buffer().set_text(&content),
                    Ok(None) => reader.buffer().set_text("Note not found."),
                    Err(err) => reader
                        .buffer()
                        .set_text(&format!("Failed to load note:\n{}", err)),
                }

                match db::get_note_tags(&conn.borrow(), &note_id) {
                    Ok(tags) => {
                        if tags.is_empty() {
                            selected_tags_label.set_text("Tags: -");
                            selected_tags_entry.set_text("");
                        } else {
                            let joined = tags.join(", ");
                            selected_tags_label.set_text(&format!("Tags: {}", joined));
                            selected_tags_entry.set_text(&joined);
                        }
                    }
                    Err(_) => {
                        selected_tags_label.set_text("Tags: error");
                    }
                }
            }
        });

        search_entry.connect_search_changed({
            let refresh_notes = Rc::clone(&refresh_notes);
            move |_| refresh_notes.as_ref()()
        });

        filter_tags_entry.connect_changed({
            let refresh_notes = Rc::clone(&refresh_notes);
            move |_| refresh_notes.as_ref()()
        });

        apply_tags_btn.connect_clicked({
            let conn = Rc::clone(&conn);
            let notes_state = Rc::clone(&notes_state);
            let list_box = list_box.clone();
            let selected_tags_entry = selected_tags_entry.clone();
            let selected_tags_label = selected_tags_label.clone();
            let refresh_notes = Rc::clone(&refresh_notes);
            move |_| {
                let Some(row) = list_box.selected_row() else {
                    return;
                };

                let index = row.index();
                if index < 0 {
                    return;
                }

                let note_id = notes_state
                    .borrow()
                    .get(index as usize)
                    .map(|n| n.id.clone());

                let Some(note_id) = note_id else {
                    return;
                };

                let tags = parse_tags(&selected_tags_entry.text());
                if db::replace_note_tags(&mut conn.borrow_mut(), &note_id, &tags).is_ok() {
                    if tags.is_empty() {
                        selected_tags_label.set_text("Tags: -");
                    } else {
                        selected_tags_label.set_text(&format!("Tags: {}", tags.join(", ")));
                    }
                    refresh_notes.as_ref()();
                }
            }
        });

        save_note_btn.connect_clicked({
            let conn = Rc::clone(&conn);
            let notes_state = Rc::clone(&notes_state);
            let list_box = list_box.clone();
            let reader = reader.clone();
            let refresh_notes = Rc::clone(&refresh_notes);
            move |_| {
                let Some(row) = list_box.selected_row() else {
                    return;
                };

                let index = row.index();
                if index < 0 {
                    return;
                }

                let note_id = notes_state
                    .borrow()
                    .get(index as usize)
                    .map(|n| n.id.clone());

                let Some(note_id) = note_id else {
                    return;
                };

                let buffer = reader.buffer();
                let start = buffer.start_iter();
                let end = buffer.end_iter();
                let content = buffer.text(&start, &end, true).to_string();

                if db::update_note_content(&mut conn.borrow_mut(), &note_id, content.trim()).is_ok()
                {
                    let _ = Notification::new()
                        .summary("Memo-Tori")
                        .body("Note updated")
                        .show();
                    refresh_notes.as_ref()();
                }
            }
        });

        refresh_notes.as_ref()();

        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed({
            let on_save = on_save.clone();
            let text_view = text_view.clone();
            move |_, key, _, state| {
                if key == gdk::Key::Return && !state.contains(gdk::ModifierType::SHIFT_MASK) {
                    on_save();
                    return Propagation::Stop;
                }

                if key == gdk::Key::Escape {
                    text_view.buffer().set_text("");
                    return Propagation::Stop;
                }

                Propagation::Proceed
            }
        });
        text_view.add_controller(key_controller);

        let nav_controller = gtk::EventControllerKey::new();
        nav_controller.connect_key_pressed({
            let stack = stack.clone();
            move |_, key, _, state| {
                if state.contains(gdk::ModifierType::CONTROL_MASK) && key == gdk::Key::Tab {
                    let current = stack.visible_child_name();
                    if current.as_deref() == Some("capture") {
                        stack.set_visible_child_name("notes");
                    } else {
                        stack.set_visible_child_name("capture");
                    }
                    return Propagation::Stop;
                }

                Propagation::Proceed
            }
        });
        window.add_controller(nav_controller);

        window.connect_close_request(move |win| {
            if quit_on_close {
                Propagation::Proceed
            } else {
                win.hide();
                Propagation::Stop
            }
        });

        window.present();
    });

    app.run();
    Ok(())
}
