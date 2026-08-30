use std::fs;
use std::path::Path;

/// Nebula Sans (SIL OFL 1.1) bundled with the app so it renders correctly
/// without requiring a manual system install. See assets/fonts/nebula-sans/OFL.txt.
const NEBULA_SANS_FILES: &[(&str, &[u8])] = &[
    (
        "NebulaSans-Book.ttf",
        include_bytes!("../assets/fonts/nebula-sans/NebulaSans-Book.ttf"),
    ),
    (
        "NebulaSans-BookItalic.ttf",
        include_bytes!("../assets/fonts/nebula-sans/NebulaSans-BookItalic.ttf"),
    ),
    (
        "NebulaSans-Medium.ttf",
        include_bytes!("../assets/fonts/nebula-sans/NebulaSans-Medium.ttf"),
    ),
    (
        "NebulaSans-MediumItalic.ttf",
        include_bytes!("../assets/fonts/nebula-sans/NebulaSans-MediumItalic.ttf"),
    ),
    (
        "NebulaSans-Semibold.ttf",
        include_bytes!("../assets/fonts/nebula-sans/NebulaSans-Semibold.ttf"),
    ),
    (
        "NebulaSans-SemiboldItalic.ttf",
        include_bytes!("../assets/fonts/nebula-sans/NebulaSans-SemiboldItalic.ttf"),
    ),
    (
        "NebulaSans-Bold.ttf",
        include_bytes!("../assets/fonts/nebula-sans/NebulaSans-Bold.ttf"),
    ),
    (
        "NebulaSans-BoldItalic.ttf",
        include_bytes!("../assets/fonts/nebula-sans/NebulaSans-BoldItalic.ttf"),
    ),
];

/// Installs the bundled fonts under the user's XDG font directory if not
/// already present, then refreshes fontconfig's cache so Pango can resolve
/// them immediately in this session.
///
/// Best-effort: any failure here just means the CSS font-family fallback
/// chain picks the next available font, so errors are swallowed.
pub fn ensure_installed() {
    let Some(data_home) = dirs::data_dir() else {
        return;
    };

    let font_dir = data_home.join("fonts").join("memo-tori");
    let Ok(()) = fs::create_dir_all(&font_dir) else {
        return;
    };

    let mut installed_new = false;
    for (filename, bytes) in NEBULA_SANS_FILES {
        let target = font_dir.join(filename);
        if needs_write(&target, bytes) {
            if fs::write(&target, bytes).is_ok() {
                installed_new = true;
            }
        }
    }

    if installed_new {
        let _ = std::process::Command::new("fc-cache")
            .arg("-f")
            .arg(&font_dir)
            .output();
    }
}

fn needs_write(target: &Path, bytes: &[u8]) -> bool {
    match fs::metadata(target) {
        Ok(meta) => meta.len() != bytes.len() as u64,
        Err(_) => true,
    }
}
