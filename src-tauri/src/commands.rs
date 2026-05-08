use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct PdfInfo {
    pub path: String,
    pub filename: String,
    pub width_pt: f32,
    pub height_pt: f32,
}

#[derive(Deserialize)]
pub struct ConflictInput {
    pub path: String,
}

#[tauri::command]
pub async fn open_pdfs(paths: Vec<String>) -> Result<Vec<PdfInfo>, String> {
    let mut results = Vec::new();

    for path in paths {
        let filename = std::path::Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let (width, height) =
            crate::pdf::get_page_dimensions(&bytes).map_err(|e| e.to_string())?;

        results.push(PdfInfo {
            path,
            filename,
            width_pt: width,
            height_pt: height,
        });
    }

    Ok(results)
}

#[tauri::command]
pub async fn render_page(path: String, width: u16) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let png = crate::pdf::render_page_to_png(&bytes, width).map_err(|e| e.to_string())?;
    Ok(png)
}

#[tauri::command]
pub async fn read_file_bytes(path: String) -> Result<Vec<u8>, String> {
    std::fs::read(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_output_conflicts(
    inputs: Vec<ConflictInput>,
    output_dir: String,
) -> Result<Vec<crate::pdf::ConflictEntry>, String> {
    let paths: Vec<String> = inputs.into_iter().map(|i| i.path).collect();
    Ok(crate::pdf::check_output_conflicts(&paths, &output_dir))
}

#[tauri::command]
pub async fn stamp_pdfs(
    jobs: Vec<crate::pdf::StampJob>,
    output_dir: String,
    skip_indices: Option<Vec<usize>>,
    flatten: bool,
) -> Result<Vec<String>, String> {
    let skip = skip_indices.unwrap_or_default();
    crate::pdf::run_stamp_jobs(&jobs, &output_dir, &skip, flatten).map_err(|e| e.to_string())
}
