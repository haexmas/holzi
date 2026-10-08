//! Commands around a chosen file that need no vault.

use tauri::AppHandle;

use super::picked::{self, PickedFile};

/// The name to show for a chosen file; the window never derives it from the file itself.
#[tauri::command]
pub fn picked_file_name(app: AppHandle, file: PickedFile) -> String {
    picked::display_name(&app, &file)
}
