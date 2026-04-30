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
    suffix: String,
) -> Result<Vec<crate::pdf::ConflictEntry>, String> {
    let paths: Vec<String> = inputs.into_iter().map(|i| i.path).collect();
    Ok(crate::pdf::check_output_conflicts(&paths, &output_dir, &suffix))
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn stamp_pdfs(
    paths: Vec<String>,
    stamp_type: String,
    image_path: Option<String>,
    text: Option<String>,
    font_size: Option<f32>,
    font_name: Option<String>,
    color: Option<String>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    output_dir: String,
    skip_indices: Option<Vec<usize>>,
) -> Result<Vec<String>, String> {
    let image_data = match image_path {
        Some(ref p) => Some(std::fs::read(p).map_err(|e| e.to_string())?),
        None => None,
    };

    let skip = skip_indices.unwrap_or_default();
    let txt = text.unwrap_or_else(|| "STAMP".to_string());
    let size = font_size.unwrap_or(24.0);
    let font = font_name.unwrap_or_else(|| "Helvetica".to_string());
    let rgb = color.as_deref().and_then(crate::pdf::parse_hex_color);

    crate::pdf::stamp_paths_to_disk(
        &paths,
        &skip,
        &output_dir,
        crate::pdf::STAMP_SUFFIX,
        |_idx, pdf_bytes| match stamp_type.as_str() {
            "image" => {
                let img = image_data
                    .as_ref()
                    .ok_or_else(|| crate::pdf::PdfError::StampError(
                        "No image data provided".into(),
                    ))?;
                crate::pdf::stamp_image(pdf_bytes, img, x, y, width, height)
            }
            "text" => crate::pdf::stamp_text(pdf_bytes, &txt, x, y, size, &font, rgb),
            other => Err(crate::pdf::PdfError::StampError(format!(
                "Unknown stamp type: {}",
                other
            ))),
        },
    )
    .map_err(|e| e.to_string())
}
