use serde::Serialize;

#[derive(Serialize)]
pub struct PdfInfo {
    pub path: String,
    pub filename: String,
    pub width_pt: f32,
    pub height_pt: f32,
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
) -> Result<Vec<String>, String> {
    let image_data = match image_path {
        Some(ref p) => Some(std::fs::read(p).map_err(|e| e.to_string())?),
        None => None,
    };

    let mut output_paths = Vec::new();

    for path in &paths {
        let pdf_bytes = std::fs::read(path).map_err(|e| e.to_string())?;

        let stamped = match stamp_type.as_str() {
            "image" => {
                let img = image_data
                    .as_ref()
                    .ok_or("No image data provided")?;
                crate::pdf::stamp_image(&pdf_bytes, img, x, y, width, height)
                    .map_err(|e| e.to_string())?
            }
            "text" => {
                let txt = text.as_deref().unwrap_or("STAMP");
                let size = font_size.unwrap_or(24.0);
                let font = font_name.as_deref().unwrap_or("Helvetica");
                let rgb = color
                    .as_deref()
                    .and_then(crate::pdf::parse_hex_color);
                crate::pdf::stamp_text(&pdf_bytes, txt, x, y, size, font, rgb)
                    .map_err(|e| e.to_string())?
            }
            _ => return Err(format!("Unknown stamp type: {}", stamp_type)),
        };

        let original_name = std::path::Path::new(path)
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".into());

        let output_path = format!("{}/{}-stamped.pdf", output_dir, original_name);
        std::fs::write(&output_path, &stamped).map_err(|e| e.to_string())?;
        output_paths.push(output_path);
    }

    Ok(output_paths)
}
