use std::sync::atomic::{AtomicU8, Ordering};
use tauri::{
    menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    AppHandle, Emitter,
};

use crate::{native_menu_translations, recent};

pub const EVENT_NAME: &str = "native-menu";
static LANGUAGE: AtomicU8 = AtomicU8::new(0);

const LANGUAGES: &[&str] = &[
    "en", "fr", "es", "de", "pt", "it", "zh", "ja", "ko", "ar", "hi", "id",
];

fn language() -> &'static str {
    match LANGUAGE.load(Ordering::Acquire) {
        0 => {
            let locale = std::env::var("LANG")
                .unwrap_or_default()
                .to_lowercase()
                .split(['-', '_', '.'])
                .next()
                .unwrap_or_default()
                .to_owned();
            LANGUAGES
                .iter()
                .copied()
                .find(|value| *value == locale)
                .unwrap_or("en")
        }
        stored => LANGUAGES
            .get(usize::from(stored - 1))
            .copied()
            .unwrap_or("en"),
    }
}

fn tr<'a>(language: &str, english: &'a str, french_text: &'a str) -> &'a str {
    if language == "fr" {
        french_text
    } else if language == "en" {
        english
    } else {
        native_menu_translations::get(language, english).unwrap_or(english)
    }
}

pub fn build(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let selected_language = language();
    let preferences = MenuItemBuilder::with_id(
        "preferences",
        tr(selected_language, "Preferences…", "Préférences…"),
    )
    .accelerator("CmdOrCtrl+,")
    .build(app)?;
    let new_project = MenuItemBuilder::with_id(
        "file:new",
        tr(selected_language, "New Project", "Nouveau projet"),
    )
    .accelerator("CmdOrCtrl+N")
    .build(app)?;
    let open_project =
        MenuItemBuilder::with_id("file:open", tr(selected_language, "Open…", "Ouvrir…"))
            .accelerator("CmdOrCtrl+O")
            .build(app)?;
    let import_audio = MenuItemBuilder::with_id(
        "file:import",
        tr(selected_language, "Import Audio…", "Importer de l’audio…"),
    )
    .accelerator("CmdOrCtrl+I")
    .build(app)?;
    let save_as = MenuItemBuilder::with_id(
        "file:save_as",
        tr(selected_language, "Save As…", "Enregistrer sous…"),
    )
    .accelerator("CmdOrCtrl+Shift+S")
    .build(app)?;
    let save_project =
        MenuItemBuilder::with_id("file:save", tr(selected_language, "Save", "Enregistrer"))
            .accelerator("CmdOrCtrl+S")
            .build(app)?;
    let quit = MenuItemBuilder::with_id(
        "app:quit",
        tr(selected_language, "Quit SonArcan", "Quitter SonArcan"),
    )
    .accelerator("CmdOrCtrl+Q")
    .build(app)?;

    let app_menu = SubmenuBuilder::new(app, "SonArcan")
        .about_with_text(
            tr(selected_language, "About SonArcan", "À propos de SonArcan"),
            None,
        )
        .separator()
        .item(&preferences)
        .separator()
        .services_with_text(tr(selected_language, "Services", "Services"))
        .separator()
        .hide_with_text(tr(selected_language, "Hide SonArcan", "Masquer SonArcan"))
        .hide_others_with_text(tr(selected_language, "Hide Others", "Masquer les autres"))
        .show_all_with_text(tr(selected_language, "Show All", "Tout afficher"))
        .separator()
        .item(&quit)
        .build()?;

    let mut recent_menu = SubmenuBuilder::new(
        app,
        tr(selected_language, "Open Recent", "Ouvrir un projet récent"),
    );
    let recent = recent::list();
    if recent.is_empty() {
        recent_menu = recent_menu.text(
            "recent:none",
            tr(
                selected_language,
                "No Recent Projects",
                "Aucun projet récent",
            ),
        );
    } else {
        for (index, path) in recent.iter().enumerate() {
            let label = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(tr(selected_language, "Project", "Projet"));
            recent_menu = recent_menu.text(format!("recent:{index}"), label);
        }
    }
    let recent_menu = recent_menu.build()?;

    let file_menu = SubmenuBuilder::new(app, tr(selected_language, "File", "Fichier"))
        .item(&new_project)
        .item(&open_project)
        .item(&recent_menu)
        .separator()
        .item(&save_project)
        .item(&save_as)
        .separator()
        .item(&import_audio)
        .text(
            "file:preferences",
            tr(selected_language, "Preferences…", "Préférences…"),
        )
        .text(
            "file:rename_project",
            tr(selected_language, "Rename Project…", "Renommer le projet…"),
        )
        .separator()
        .close_window_with_text(tr(selected_language, "Close Window", "Fermer la fenêtre"))
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, tr(selected_language, "Edit", "Édition"))
        .undo_with_text(tr(selected_language, "Undo", "Annuler"))
        .redo_with_text(tr(selected_language, "Redo", "Rétablir"))
        .separator()
        .cut_with_text(tr(selected_language, "Cut", "Couper"))
        .copy_with_text(tr(selected_language, "Copy", "Copier"))
        .paste_with_text(tr(selected_language, "Paste", "Coller"))
        .select_all_with_text(tr(selected_language, "Select All", "Tout sélectionner"))
        .build()?;

    let view_menu = SubmenuBuilder::new(app, tr(selected_language, "View", "Présentation"))
        .text(
            "view:console",
            tr(
                selected_language,
                "Show/Hide Console",
                "Afficher/Masquer la console",
            ),
        )
        .separator()
        .text(
            "view:zoom_in",
            tr(
                selected_language,
                "Zoom Waveform In",
                "Agrandir la forme d’onde",
            ),
        )
        .text(
            "view:zoom_out",
            tr(
                selected_language,
                "Zoom Waveform Out",
                "Réduire la forme d’onde",
            ),
        )
        .text(
            "view:zoom_reset",
            tr(
                selected_language,
                "Reset Waveform Zoom",
                "Réinitialiser le zoom",
            ),
        )
        .separator()
        .fullscreen_with_text(tr(
            selected_language,
            "Enter Full Screen",
            "Activer le plein écran",
        ))
        .build()?;

    let playback_menu = SubmenuBuilder::new(app, tr(selected_language, "Playback", "Lecture"))
        .text(
            "playback:toggle",
            tr(selected_language, "Play/Pause", "Lecture/Pause"),
        )
        .text(
            "playback:back",
            tr(
                selected_language,
                "Jump Back 5 Seconds",
                "Reculer de 5 secondes",
            ),
        )
        .text(
            "playback:forward",
            tr(
                selected_language,
                "Jump Forward 5 Seconds",
                "Avancer de 5 secondes",
            ),
        )
        .separator()
        .text(
            "playback:set_a",
            tr(selected_language, "Set Loop A", "Placer le point A"),
        )
        .text(
            "playback:set_b",
            tr(selected_language, "Set Loop B", "Placer le point B"),
        )
        .text(
            "playback:clear_loop",
            tr(selected_language, "Clear Loop", "Effacer la boucle"),
        )
        .build()?;

    let playlist_menu = SubmenuBuilder::new(app, tr(selected_language, "Songs", "Morceaux"))
        .text("playlist:add", tr(selected_language, "Add…", "Ajouter…"))
        .separator()
        .text(
            "playlist:export_json",
            tr(selected_language, "Export as JSON…", "Exporter en JSON…"),
        )
        .text(
            "playlist:export_markdown",
            tr(
                selected_language,
                "Export as Markdown…",
                "Exporter en Markdown…",
            ),
        )
        .build()?;

    let window_menu = SubmenuBuilder::new(app, tr(selected_language, "Window", "Fenêtre"))
        .item(&PredefinedMenuItem::minimize(
            app,
            Some(tr(selected_language, "Minimize", "Réduire")),
        )?)
        .item(&PredefinedMenuItem::bring_all_to_front(
            app,
            Some(tr(
                selected_language,
                "Bring All to Front",
                "Tout ramener au premier plan",
            )),
        )?)
        .build()?;

    let help_menu = SubmenuBuilder::new(app, tr(selected_language, "Help", "Aide"))
        .text(
            "help:diagnostics",
            tr(selected_language, "Diagnostics…", "Diagnostic…"),
        )
        .text(
            "help:shortcuts",
            tr(
                selected_language,
                "Keyboard Shortcuts…",
                "Raccourcis clavier…",
            ),
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
    let stored = LANGUAGES
        .iter()
        .position(|candidate| *candidate == language)
        .map(|index| u8::try_from(index + 1).unwrap_or(1))
        .unwrap_or(1);
    LANGUAGE.store(stored, Ordering::Release);
    install(app)
}

pub fn handle_event(app: &AppHandle, id: &str) {
    if id == "recent:none" {
        return;
    }
    let _ = app.emit(EVENT_NAME, id);
}
