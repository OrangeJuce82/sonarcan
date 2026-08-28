use std::sync::atomic::{AtomicU8, Ordering};
use tauri::{
    menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    AppHandle, Emitter,
};

use crate::recent;

pub const EVENT_NAME: &str = "native-menu";
static LANGUAGE: AtomicU8 = AtomicU8::new(0);

fn is_french() -> bool {
    match LANGUAGE.load(Ordering::Acquire) {
        2 => true,
        1 => false,
        _ => std::env::var("LANG")
            .unwrap_or_default()
            .to_lowercase()
            .starts_with("fr"),
    }
}

fn tr<'a>(french: bool, english: &'a str, french_text: &'a str) -> &'a str {
    if french {
        french_text
    } else {
        english
    }
}

pub fn build(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let fr = is_french();
    let preferences =
        MenuItemBuilder::with_id("preferences", tr(fr, "Preferences…", "Préférences…"))
            .accelerator("CmdOrCtrl+,")
            .build(app)?;
    let new_project = MenuItemBuilder::with_id("file:new", tr(fr, "New Project", "Nouveau projet"))
        .accelerator("CmdOrCtrl+N")
        .build(app)?;
    let open_project = MenuItemBuilder::with_id("file:open", tr(fr, "Open…", "Ouvrir…"))
        .accelerator("CmdOrCtrl+O")
        .build(app)?;
    let import_audio = MenuItemBuilder::with_id(
        "file:import",
        tr(fr, "Import Audio…", "Importer de l’audio…"),
    )
    .accelerator("CmdOrCtrl+I")
    .build(app)?;
    let save_as = MenuItemBuilder::with_id("file:save_as", tr(fr, "Save As…", "Enregistrer sous…"))
        .accelerator("CmdOrCtrl+Shift+S")
        .build(app)?;
    let save_project = MenuItemBuilder::with_id("file:save", tr(fr, "Save", "Enregistrer"))
        .accelerator("CmdOrCtrl+S")
        .build(app)?;
    let quit = MenuItemBuilder::with_id("app:quit", tr(fr, "Quit SonArcan", "Quitter SonArcan"))
        .accelerator("CmdOrCtrl+Q")
        .build(app)?;

    let app_menu = SubmenuBuilder::new(app, "SonArcan")
        .about_with_text(tr(fr, "About SonArcan", "À propos de SonArcan"), None)
        .separator()
        .item(&preferences)
        .separator()
        .services_with_text(tr(fr, "Services", "Services"))
        .separator()
        .hide_with_text(tr(fr, "Hide SonArcan", "Masquer SonArcan"))
        .hide_others_with_text(tr(fr, "Hide Others", "Masquer les autres"))
        .show_all_with_text(tr(fr, "Show All", "Tout afficher"))
        .separator()
        .item(&quit)
        .build()?;

    let mut recent_menu =
        SubmenuBuilder::new(app, tr(fr, "Open Recent", "Ouvrir un projet récent"));
    let recent = recent::list();
    if recent.is_empty() {
        recent_menu = recent_menu.text(
            "recent:none",
            tr(fr, "No Recent Projects", "Aucun projet récent"),
        );
    } else {
        for (index, path) in recent.iter().enumerate() {
            let label = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(tr(fr, "Project", "Projet"));
            recent_menu = recent_menu.text(format!("recent:{index}"), label);
        }
    }
    let recent_menu = recent_menu.build()?;

    let file_menu = SubmenuBuilder::new(app, tr(fr, "File", "Fichier"))
        .item(&new_project)
        .item(&open_project)
        .item(&recent_menu)
        .separator()
        .item(&save_project)
        .item(&save_as)
        .separator()
        .item(&import_audio)
        .text("file:preferences", tr(fr, "Preferences…", "Préférences…"))
        .text(
            "file:rename_project",
            tr(fr, "Rename Project…", "Renommer le projet…"),
        )
        .separator()
        .close_window_with_text(tr(fr, "Close Window", "Fermer la fenêtre"))
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, tr(fr, "Edit", "Édition"))
        .undo_with_text(tr(fr, "Undo", "Annuler"))
        .redo_with_text(tr(fr, "Redo", "Rétablir"))
        .separator()
        .cut_with_text(tr(fr, "Cut", "Couper"))
        .copy_with_text(tr(fr, "Copy", "Copier"))
        .paste_with_text(tr(fr, "Paste", "Coller"))
        .select_all_with_text(tr(fr, "Select All", "Tout sélectionner"))
        .build()?;

    let view_menu = SubmenuBuilder::new(app, tr(fr, "View", "Présentation"))
        .text(
            "view:console",
            tr(fr, "Show/Hide Console", "Afficher/Masquer la console"),
        )
        .separator()
        .text(
            "view:zoom_in",
            tr(fr, "Zoom Waveform In", "Agrandir la forme d’onde"),
        )
        .text(
            "view:zoom_out",
            tr(fr, "Zoom Waveform Out", "Réduire la forme d’onde"),
        )
        .text(
            "view:zoom_reset",
            tr(fr, "Reset Waveform Zoom", "Réinitialiser le zoom"),
        )
        .separator()
        .fullscreen_with_text(tr(fr, "Enter Full Screen", "Activer le plein écran"))
        .build()?;

    let playback_menu = SubmenuBuilder::new(app, tr(fr, "Playback", "Lecture"))
        .text("playback:toggle", tr(fr, "Play/Pause", "Lecture/Pause"))
        .text(
            "playback:back",
            tr(fr, "Jump Back 5 Seconds", "Reculer de 5 secondes"),
        )
        .text(
            "playback:forward",
            tr(fr, "Jump Forward 5 Seconds", "Avancer de 5 secondes"),
        )
        .separator()
        .text("playback:set_a", tr(fr, "Set Loop A", "Placer le point A"))
        .text("playback:set_b", tr(fr, "Set Loop B", "Placer le point B"))
        .text(
            "playback:clear_loop",
            tr(fr, "Clear Loop", "Effacer la boucle"),
        )
        .build()?;

    let playlist_menu = SubmenuBuilder::new(app, tr(fr, "Songs", "Morceaux"))
        .text("playlist:add", tr(fr, "Add…", "Ajouter…"))
        .separator()
        .text(
            "playlist:export_json",
            tr(fr, "Export as JSON…", "Exporter en JSON…"),
        )
        .text(
            "playlist:export_markdown",
            tr(fr, "Export as Markdown…", "Exporter en Markdown…"),
        )
        .build()?;

    let window_menu = SubmenuBuilder::new(app, tr(fr, "Window", "Fenêtre"))
        .item(&PredefinedMenuItem::minimize(
            app,
            Some(tr(fr, "Minimize", "Réduire")),
        )?)
        .item(&PredefinedMenuItem::bring_all_to_front(
            app,
            Some(tr(fr, "Bring All to Front", "Tout ramener au premier plan")),
        )?)
        .build()?;

    let help_menu = SubmenuBuilder::new(app, tr(fr, "Help", "Aide"))
        .text("help:diagnostics", tr(fr, "Diagnostics…", "Diagnostic…"))
        .text(
            "help:shortcuts",
            tr(fr, "Keyboard Shortcuts…", "Raccourcis clavier…"),
        )
        .build()?;

    MenuBuilder::new(app)
        .items(&[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &playlist_menu,
            &playback_menu,
            &window_menu,
            &help_menu,
        ])
        .build()
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    app.set_menu(build(app)?)?;
    Ok(())
}

pub fn set_language(app: &AppHandle, language: &str) -> tauri::Result<()> {
    LANGUAGE.store(if language == "fr" { 2 } else { 1 }, Ordering::Release);
    install(app)
}

pub fn handle_event(app: &AppHandle, id: &str) {
    if id == "recent:none" {
        return;
    }
    let _ = app.emit(EVENT_NAME, id);
}
