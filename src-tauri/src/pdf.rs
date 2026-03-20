use image::GenericImageView;
use lopdf::content::{Content, Operation};
use lopdf::Object::Name;
use lopdf::{Dictionary, Document, Object, Stream};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PdfError {
    #[error("failed to load PDF: {0}")]
    LoadError(String),
    #[error("failed to render page: {0}")]
    RenderError(String),
    #[error("failed to stamp PDF: {0}")]
    StampError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Parse a PDF object as f32, handling both Real and Integer types.
fn obj_as_f32(obj: &Object) -> Option<f32> {
    obj.as_float().ok().map(|v| v as f32)
        .or_else(|| obj.as_i64().ok().map(|i| i as f32))
}

/// Extract an array value from a dictionary, resolving indirect references.
fn resolve_array<'a>(doc: &'a Document, dict: &'a Dictionary, key: &[u8]) -> Option<Vec<&'a Object>> {
    match dict.get(key).ok()? {
        Object::Array(arr) => Some(arr.iter().collect()),
        Object::Reference(id) => {
            if let Ok(Object::Array(arr)) = doc.get_object(*id) {
                Some(arr.iter().collect())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extract an integer value from a dictionary, resolving indirect references.
fn resolve_int(doc: &Document, dict: &Dictionary, key: &[u8]) -> Option<i64> {
    match dict.get(key).ok()? {
        Object::Integer(i) => Some(*i),
        Object::Reference(id) => {
            if let Ok(Object::Integer(i)) = doc.get_object(*id) {
                Some(*i)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Page geometry info: raw MediaBox dimensions, rotation, and effective (as-displayed) dimensions.
pub struct PageGeometry {
    /// Raw MediaBox width (before rotation).
    pub raw_width: f32,
    /// Raw MediaBox height (before rotation).
    pub raw_height: f32,
    /// Page rotation in degrees (0, 90, 180, 270).
    pub rotation: u32,
    /// Effective width as displayed (after applying rotation).
    pub eff_width: f32,
    /// Effective height as displayed (after applying rotation).
    pub eff_height: f32,
}

/// Extract page geometry from the first page, handling inherited MediaBox and /Rotate.
pub fn get_page_geometry(pdf_bytes: &[u8]) -> Result<PageGeometry, PdfError> {
    let doc = Document::load_mem(pdf_bytes)
        .map_err(|e| PdfError::LoadError(e.to_string()))?;

    let page_id = doc
        .page_iter()
        .next()
        .ok_or_else(|| PdfError::LoadError("PDF has no pages".into()))?;

    let page = doc
        .get_object(page_id)
        .map_err(|e| PdfError::LoadError(e.to_string()))?;

    let page_dict = page
        .as_dict()
        .map_err(|e| PdfError::LoadError(e.to_string()))?;

    // Walk up the page tree to find inherited MediaBox and Rotate
    let mut media_box_arr = resolve_array(&doc, page_dict, b"MediaBox");
    let mut rotation = resolve_int(&doc, page_dict, b"Rotate");

    // If not found on the page itself, check parent nodes
    let mut parent_ref = page_dict.get(b"Parent").ok().and_then(|o| {
        if let Object::Reference(id) = o { Some(*id) } else { None }
    });

    while (media_box_arr.is_none() || rotation.is_none()) && parent_ref.is_some() {
        let pid = parent_ref.unwrap();
        if let Ok(parent_obj) = doc.get_object(pid) {
            if let Ok(parent_dict) = parent_obj.as_dict() {
                if media_box_arr.is_none() {
                    media_box_arr = resolve_array(&doc, parent_dict, b"MediaBox");
                }
                if rotation.is_none() {
                    rotation = resolve_int(&doc, parent_dict, b"Rotate");
                }
                parent_ref = parent_dict.get(b"Parent").ok().and_then(|o| {
                    if let Object::Reference(id) = o { Some(*id) } else { None }
                });
            } else {
                break;
            }
        } else {
            break;
        }
    }

    let (raw_width, raw_height) = if let Some(arr) = media_box_arr {
        if arr.len() >= 4 {
            let x1 = obj_as_f32(arr[0]).unwrap_or(0.0);
            let y1 = obj_as_f32(arr[1]).unwrap_or(0.0);
            let x2 = obj_as_f32(arr[2]).unwrap_or(612.0);
            let y2 = obj_as_f32(arr[3]).unwrap_or(792.0);
            ((x2 - x1).abs(), (y2 - y1).abs())
        } else {
            (612.0, 792.0)
        }
    } else {
        (612.0, 792.0)
    };

    let rotation = (rotation.unwrap_or(0) % 360 + 360) as u32 % 360;

    let (eff_width, eff_height) = if rotation == 90 || rotation == 270 {
        (raw_height, raw_width)
    } else {
        (raw_width, raw_height)
    };

    Ok(PageGeometry { raw_width, raw_height, rotation, eff_width, eff_height })
}

/// Convenience wrapper returning effective (displayed) dimensions.
pub fn get_page_dimensions(pdf_bytes: &[u8]) -> Result<(f32, f32), PdfError> {
    let geo = get_page_geometry(pdf_bytes)?;
    Ok((geo.eff_width, geo.eff_height))
}

/// Render the first page of a PDF to PNG bytes.
pub fn render_page_to_png(pdf_bytes: &[u8], target_width: u16) -> Result<Vec<u8>, PdfError> {
    use pdfium_render::prelude::*;

    let lib_path = pdfium_lib_path();
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(
            &lib_path,
        ))
        .map_err(|e| PdfError::RenderError(e.to_string()))?,
    );

    let doc = pdfium
        .load_pdf_from_byte_slice(pdf_bytes, None)
        .map_err(|e| PdfError::RenderError(e.to_string()))?;

    let page = doc
        .pages()
        .get(0)
        .map_err(|e| PdfError::RenderError(e.to_string()))?;

    let config = PdfRenderConfig::new().set_target_width(target_width as i32);

    let bitmap = page
        .render_with_config(&config)
        .map_err(|e| PdfError::RenderError(e.to_string()))?;

    let img = bitmap.as_image();
    let mut png_bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_bytes),
        image::ImageFormat::Png,
    )
    .map_err(|e| PdfError::RenderError(e.to_string()))?;

    Ok(png_bytes)
}

/// Compute the `cm` matrix [a, b, c, d, e, f] for placing an image stamp,
/// transforming from effective (displayed) coordinates to raw (unrotated) page space.
///
/// The image maps from unit square [0,0]-[1,1] to the target rectangle.
/// For rotated pages the stamp must be counter-rotated so it appears upright
/// in the displayed (rotated) view.
fn image_cm_for_rotation(
    rotation: u32,
    dx: f32, dy: f32,
    sw: f32, sh: f32,
    raw_w: f32, raw_h: f32,
) -> [f32; 6] {
    match rotation {
        90  => [0.0,  sw, -sh, 0.0, raw_w - dy, dx],
        180 => [-sw, 0.0, 0.0, -sh, raw_w - dx, raw_h - dy],
        270 => [0.0, -sw,  sh, 0.0, dy, raw_h - dx],
        _   => [sw,  0.0, 0.0,  sh, dx, dy], // 0° — identity orientation
    }
}

/// Compute the coordinate-space `cm` matrix that transforms display-space
/// coordinates into raw (unrotated) page-space coordinates.
/// Used for text stamps where position is set via `Td` in display coords.
fn coord_cm_for_rotation(rotation: u32, raw_w: f32, raw_h: f32) -> Option<[f32; 6]> {
    match rotation {
        90  => Some([0.0,  1.0, -1.0, 0.0, raw_w, 0.0]),
        180 => Some([-1.0, 0.0, 0.0, -1.0, raw_w, raw_h]),
        270 => Some([0.0, -1.0,  1.0, 0.0, 0.0,   raw_h]),
        _   => None, // 0° needs no extra transform
    }
}

/// Overlay an image stamp on the first page of a PDF.
/// Uses a self-contained Form XObject so we only need to register one name
/// in the page's Resources, and the image lives inside the Form's own Resources.
///
/// Coordinates (x, y, width, height) are in the **effective** (as-displayed)
/// coordinate system — the same space the preview uses.
pub fn stamp_image(
    pdf_bytes: &[u8],
    image_bytes: &[u8],
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<Vec<u8>, PdfError> {
    let mut doc = Document::load_mem(pdf_bytes)
        .map_err(|e| PdfError::StampError(e.to_string()))?;

    let page_id = doc
        .page_iter()
        .next()
        .ok_or_else(|| PdfError::StampError("PDF has no pages".into()))?;

    let geo = get_page_geometry(pdf_bytes)?;

    // Create image XObject manually (lopdf's image_from mishandles RGBA)
    let img_id = create_image_xobject(&mut doc, image_bytes)?;

    // Build a Form XObject that draws the image, carrying its own Resources.
    // The cm matrix accounts for page rotation so the stamp appears at the
    // correct position and orientation in the displayed (rotated) page.
    let img_ref_name = b"Img0";
    let [a, b, c, d, e, f] = image_cm_for_rotation(
        geo.rotation, x, y, width, height, geo.raw_width, geo.raw_height,
    );
    let form_ops = vec![
        Operation::new("q", vec![]),
        Operation::new(
            "cm",
            vec![a.into(), b.into(), c.into(), d.into(), e.into(), f.into()],
        ),
        Operation::new("Do", vec![Name(img_ref_name.to_vec())]),
        Operation::new("Q", vec![]),
    ];
    let form_content = Content { operations: form_ops }
        .encode()
        .map_err(|e| PdfError::StampError(e.to_string()))?;

    // Form BBox uses raw (unrotated) page dimensions — form lives in page space
    let mut xobjects = Dictionary::new();
    xobjects.set(img_ref_name.as_slice(), Object::Reference(img_id));
    let mut resources = Dictionary::new();
    resources.set("XObject", Object::Dictionary(xobjects));

    let mut form_dict = Dictionary::new();
    form_dict.set("Type", Name(b"XObject".to_vec()));
    form_dict.set("Subtype", Name(b"Form".to_vec()));
    form_dict.set(
        "BBox",
        Object::Array(vec![
            0.0f32.into(), 0.0f32.into(),
            geo.raw_width.into(), geo.raw_height.into(),
        ]),
    );
    form_dict.set("Resources", Object::Dictionary(resources));

    let form_id = doc.add_object(Stream::new(form_dict, form_content));
    let form_name = format!("Stamp{}", form_id.0);

    // Register the Form XObject in the page's Resources
    register_xobject(&mut doc, page_id, form_name.as_bytes(), form_id)?;

    // Append a content stream that invokes the Form XObject
    let page_ops = vec![
        Operation::new("q", vec![]),
        Operation::new("Do", vec![Name(form_name.into_bytes())]),
        Operation::new("Q", vec![]),
    ];
    let page_content = Content { operations: page_ops }
        .encode()
        .map_err(|e| PdfError::StampError(e.to_string()))?;
    let stamp_stream_id = doc.add_object(Stream::new(Dictionary::new(), page_content));

    append_content_stream(&mut doc, page_id, stamp_stream_id)?;

    let mut output = Vec::new();
    doc.save_to(&mut output)
        .map_err(|e| PdfError::StampError(e.to_string()))?;
    Ok(output)
}

/// Overlay a text stamp on the first page of a PDF.
/// Uses a self-contained Form XObject with its own font Resources.
///
/// Coordinates (x, y) are in the **effective** (as-displayed) coordinate system.
pub fn stamp_text(
    pdf_bytes: &[u8],
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    font_name: &str,
    color: Option<(f32, f32, f32)>,
) -> Result<Vec<u8>, PdfError> {
    let mut doc = Document::load_mem(pdf_bytes)
        .map_err(|e| PdfError::StampError(e.to_string()))?;

    let page_id = doc
        .page_iter()
        .next()
        .ok_or_else(|| PdfError::StampError("PDF has no pages".into()))?;

    let geo = get_page_geometry(pdf_bytes)?;

    // Create font object
    let mut font_dict = Dictionary::new();
    font_dict.set("Type", Name(b"Font".to_vec()));
    font_dict.set("Subtype", Name(b"Type1".to_vec()));
    font_dict.set("BaseFont", Name(font_name.as_bytes().to_vec()));
    let font_id = doc.add_object(Object::Dictionary(font_dict));

    // Build text content stream
    let font_ref_name = b"F1";
    let mut ops = Vec::new();
    ops.push(Operation::new("q", vec![]));

    // For rotated pages, apply a coordinate transform so that text position
    // and orientation in display space map correctly to raw page space.
    if let Some([a, b, c, d, e, f]) = coord_cm_for_rotation(
        geo.rotation, geo.raw_width, geo.raw_height,
    ) {
        ops.push(Operation::new(
            "cm",
            vec![a.into(), b.into(), c.into(), d.into(), e.into(), f.into()],
        ));
    }

    if let Some((r, g, b)) = color {
        ops.push(Operation::new("rg", vec![r.into(), g.into(), b.into()]));
    }
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new(
        "Tf",
        vec![Name(font_ref_name.to_vec()), font_size.into()],
    ));
    ops.push(Operation::new("Td", vec![x.into(), y.into()]));
    ops.push(Operation::new("Tj", vec![Object::string_literal(text)]));
    ops.push(Operation::new("ET", vec![]));
    ops.push(Operation::new("Q", vec![]));

    let form_content = Content { operations: ops }
        .encode()
        .map_err(|e| PdfError::StampError(e.to_string()))?;

    // Form BBox uses raw (unrotated) page dimensions
    let mut fonts = Dictionary::new();
    fonts.set(font_ref_name.as_slice(), Object::Reference(font_id));
    let mut resources = Dictionary::new();
    resources.set("Font", Object::Dictionary(fonts));

    let mut form_dict = Dictionary::new();
    form_dict.set("Type", Name(b"XObject".to_vec()));
    form_dict.set("Subtype", Name(b"Form".to_vec()));
    form_dict.set(
        "BBox",
        Object::Array(vec![
            0.0f32.into(), 0.0f32.into(),
            geo.raw_width.into(), geo.raw_height.into(),
        ]),
    );
    form_dict.set("Resources", Object::Dictionary(resources));

    let form_id = doc.add_object(Stream::new(form_dict, form_content));
    let form_name = format!("Stamp{}", form_id.0);

    // Register the Form XObject in the page's Resources
    register_xobject(&mut doc, page_id, form_name.as_bytes(), form_id)?;

    // Append a content stream that invokes the Form
    let page_ops = vec![
        Operation::new("q", vec![]),
        Operation::new("Do", vec![Name(form_name.into_bytes())]),
        Operation::new("Q", vec![]),
    ];
    let page_content = Content { operations: page_ops }
        .encode()
        .map_err(|e| PdfError::StampError(e.to_string()))?;
    let stamp_stream_id = doc.add_object(Stream::new(Dictionary::new(), page_content));

    append_content_stream(&mut doc, page_id, stamp_stream_id)?;

    let mut output = Vec::new();
    doc.save_to(&mut output)
        .map_err(|e| PdfError::StampError(e.to_string()))?;
    Ok(output)
}

/// Register an XObject name in the page's Resources, properly handling
/// indirect references at every level (Resources, XObject sub-dict).
fn register_xobject(
    doc: &mut Document,
    page_id: lopdf::ObjectId,
    name: &[u8],
    xobject_id: lopdf::ObjectId,
) -> Result<(), PdfError> {
    // Step 1: Find if the page's Resources is an indirect reference
    let resources_ref = {
        let page = doc
            .get_object(page_id)
            .map_err(|e| PdfError::StampError(e.to_string()))?;
        let page_dict = page
            .as_dict()
            .map_err(|e| PdfError::StampError(e.to_string()))?;
        match page_dict.get(b"Resources") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        }
    };

    // Step 2: Get a mutable reference to the Resources dictionary
    if let Some(res_id) = resources_ref {
        // Resources is an indirect object — modify it directly
        let xobj_ref = {
            let res = doc
                .get_object(res_id)
                .map_err(|e| PdfError::StampError(e.to_string()))?;
            let res_dict = res
                .as_dict()
                .map_err(|e| PdfError::StampError(e.to_string()))?;
            match res_dict.get(b"XObject") {
                Ok(Object::Reference(id)) => Some(*id),
                _ => None,
            }
        };

        if let Some(xobj_dict_id) = xobj_ref {
            // XObject is also an indirect reference
            let xobj = doc
                .get_object_mut(xobj_dict_id)
                .map_err(|e| PdfError::StampError(e.to_string()))?;
            let xobj_dict = xobj
                .as_dict_mut()
                .map_err(|e| PdfError::StampError(e.to_string()))?;
            xobj_dict.set(name, Object::Reference(xobject_id));
        } else {
            // XObject is inline or missing — modify via Resources
            let res = doc
                .get_object_mut(res_id)
                .map_err(|e| PdfError::StampError(e.to_string()))?;
            let res_dict = res
                .as_dict_mut()
                .map_err(|e| PdfError::StampError(e.to_string()))?;
            if !res_dict.has(b"XObject") {
                res_dict.set("XObject", Object::Dictionary(Dictionary::new()));
            }
            let xobj = res_dict
                .get_mut(b"XObject")
                .map_err(|e| PdfError::StampError(e.to_string()))?;
            let xobj_dict = xobj
                .as_dict_mut()
                .map_err(|e| PdfError::StampError(e.to_string()))?;
            xobj_dict.set(name, Object::Reference(xobject_id));
        }
    } else {
        // Resources is inline or missing — modify the page dict directly
        let page = doc
            .get_object_mut(page_id)
            .map_err(|e| PdfError::StampError(e.to_string()))?;
        let page_dict = page
            .as_dict_mut()
            .map_err(|e| PdfError::StampError(e.to_string()))?;
        if !page_dict.has(b"Resources") {
            page_dict.set("Resources", Object::Dictionary(Dictionary::new()));
        }
        let resources = page_dict
            .get_mut(b"Resources")
            .map_err(|e| PdfError::StampError(e.to_string()))?;
        let res_dict = resources
            .as_dict_mut()
            .map_err(|e| PdfError::StampError(e.to_string()))?;
        if !res_dict.has(b"XObject") {
            res_dict.set("XObject", Object::Dictionary(Dictionary::new()));
        }
        let xobj = res_dict
            .get_mut(b"XObject")
            .map_err(|e| PdfError::StampError(e.to_string()))?;
        let xobj_dict = xobj
            .as_dict_mut()
            .map_err(|e| PdfError::StampError(e.to_string()))?;
        xobj_dict.set(name, Object::Reference(xobject_id));
    }

    Ok(())
}

/// Append a content stream object to the page's Contents entry.
/// If Contents is a single reference, converts it to an array first.
fn append_content_stream(
    doc: &mut Document,
    page_id: lopdf::ObjectId,
    stream_id: lopdf::ObjectId,
) -> Result<(), PdfError> {
    let page = doc
        .get_object_mut(page_id)
        .map_err(|e| PdfError::StampError(e.to_string()))?;
    let page_dict = page
        .as_dict_mut()
        .map_err(|e| PdfError::StampError(e.to_string()))?;

    match page_dict.get(b"Contents") {
        Ok(Object::Reference(existing_id)) => {
            let existing_id = *existing_id;
            page_dict.set(
                "Contents",
                Object::Array(vec![
                    Object::Reference(existing_id),
                    Object::Reference(stream_id),
                ]),
            );
        }
        Ok(Object::Array(arr)) => {
            let mut new_arr = arr.clone();
            new_arr.push(Object::Reference(stream_id));
            page_dict.set("Contents", Object::Array(new_arr));
        }
        _ => {
            page_dict.set("Contents", Object::Reference(stream_id));
        }
    }

    Ok(())
}

/// Parse a hex color string like "#ff0000" into (r, g, b) floats in [0, 1].
pub fn parse_hex_color(hex: &str) -> Option<(f32, f32, f32)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
}

/// Build a proper image XObject from raw image file bytes (PNG, JPEG, etc.).
/// Handles RGBA by stripping alpha (or using SMask) and always produces
/// a valid DeviceRGB or DeviceGray XObject.
fn create_image_xobject(
    doc: &mut Document,
    file_bytes: &[u8],
) -> Result<lopdf::ObjectId, PdfError> {
    // Check if JPEG — can embed directly with DCTDecode
    let is_jpeg = file_bytes.starts_with(&[0xFF, 0xD8, 0xFF]);

    if is_jpeg {
        let img = image::load_from_memory(file_bytes)
            .map_err(|e| PdfError::StampError(e.to_string()))?;
        let (w, h) = img.dimensions();
        let mut dict = Dictionary::new();
        dict.set("Type", Name(b"XObject".to_vec()));
        dict.set("Subtype", Name(b"Image".to_vec()));
        dict.set("Width", Object::Integer(w as i64));
        dict.set("Height", Object::Integer(h as i64));
        dict.set("ColorSpace", Name(b"DeviceRGB".to_vec()));
        dict.set("BitsPerComponent", Object::Integer(8));
        dict.set("Filter", Name(b"DCTDecode".to_vec()));
        let stream = Stream::new(dict, file_bytes.to_vec());
        return Ok(doc.add_object(stream));
    }

    // For PNG and other formats: decode, convert to RGB8, compress with Flate
    let img = image::load_from_memory(file_bytes)
        .map_err(|e| PdfError::StampError(e.to_string()))?;
    let (w, h) = img.dimensions();

    // Convert to RGB (strips alpha if present)
    let rgb_img = img.to_rgb8();
    let rgb_bytes = rgb_img.into_raw();

    let mut dict = Dictionary::new();
    dict.set("Type", Name(b"XObject".to_vec()));
    dict.set("Subtype", Name(b"Image".to_vec()));
    dict.set("Width", Object::Integer(w as i64));
    dict.set("Height", Object::Integer(h as i64));
    dict.set("ColorSpace", Name(b"DeviceRGB".to_vec()));
    dict.set("BitsPerComponent", Object::Integer(8));

    let mut stream = Stream::new(dict, rgb_bytes);
    stream
        .compress()
        .map_err(|e| PdfError::StampError(e.to_string()))?;

    Ok(doc.add_object(stream))
}

fn pdfium_lib_path() -> String {
    // Dev mode: source directory
    let dev_path = concat!(env!("CARGO_MANIFEST_DIR"), "/libs/pdfium/lib");
    if std::path::Path::new(dev_path).exists() {
        return dev_path.to_string();
    }

    // Production: resolve relative to the executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // macOS: binary is at APP.app/Contents/MacOS/exe, resources at ../Resources/
            let macos_path = exe_dir.join("../Resources/libs/pdfium/lib");
            if macos_path.exists() {
                return macos_path.to_string_lossy().to_string();
            }
            // Windows: resources next to exe
            let win_path = exe_dir.join("libs/pdfium/lib");
            if win_path.exists() {
                return win_path.to_string_lossy().to_string();
            }
        }
    }

    dev_path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helpers -----------------------------------------------------------

    /// Create a minimal valid single-page PDF with the given MediaBox and optional /Rotate.
    fn make_test_pdf(width: f32, height: f32, rotate: Option<u32>) -> Vec<u8> {
        let mut doc = Document::new();

        // A minimal content stream (empty page)
        let content = Stream::new(Dictionary::new(), b"".to_vec());
        let content_id = doc.add_object(content);

        // Build the page dictionary
        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Name(b"Page".to_vec()));
        page_dict.set(
            "MediaBox",
            Object::Array(vec![
                0.0f32.into(), 0.0f32.into(), width.into(), height.into(),
            ]),
        );
        page_dict.set("Contents", Object::Reference(content_id));
        if let Some(r) = rotate {
            page_dict.set("Rotate", Object::Integer(r as i64));
        }

        let page_id = doc.add_object(Object::Dictionary(page_dict));

        // Build the Pages node
        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Name(b"Pages".to_vec()));
        pages_dict.set("Count", Object::Integer(1));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        let pages_id = doc.add_object(Object::Dictionary(pages_dict));

        // Set parent reference on the page
        let page = doc.get_object_mut(page_id).unwrap();
        page.as_dict_mut().unwrap().set("Parent", Object::Reference(pages_id));

        // Build the catalog
        let mut catalog = Dictionary::new();
        catalog.set("Type", Name(b"Catalog".to_vec()));
        catalog.set("Pages", Object::Reference(pages_id));
        let catalog_id = doc.add_object(Object::Dictionary(catalog));

        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }

    /// Create a tiny 2x2 red PNG for stamp tests.
    fn make_red_png() -> Vec<u8> {
        let img = image::RgbImage::from_fn(2, 2, |_, _| image::Rgb([255, 0, 0]));
        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        bytes
    }

    /// Extract the cm matrix values from the Form XObject's content stream in a stamped PDF.
    fn extract_cm_from_stamped(pdf_bytes: &[u8]) -> Vec<f32> {
        let doc = Document::load_mem(pdf_bytes).unwrap();
        let page_id = doc.page_iter().next().unwrap();
        let page = doc.get_object(page_id).unwrap();
        let page_dict = page.as_dict().unwrap();

        // Find the stamp Form XObject in Resources
        let res = match page_dict.get(b"Resources").unwrap() {
            Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
            Object::Dictionary(d) => d.clone(),
            _ => panic!("unexpected Resources type"),
        };
        let xobjects = match res.get(b"XObject").unwrap() {
            Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
            Object::Dictionary(d) => d.clone(),
            _ => panic!("unexpected XObject type"),
        };

        // Find the Stamp* entry
        for (name, val) in xobjects.iter() {
            let name_str = std::str::from_utf8(name).unwrap_or("");
            if name_str.starts_with("Stamp") {
                if let Object::Reference(id) = val {
                    if let Ok(Object::Stream(s)) = doc.get_object(*id) {
                        let mut sc = s.clone();
                        let _ = sc.decompress();
                        let txt = String::from_utf8_lossy(&sc.content);
                        // Parse "... a b c d e f cm ..."
                        let parts: Vec<&str> = txt.split_whitespace().collect();
                        for (i, &part) in parts.iter().enumerate() {
                            if part == "cm" && i >= 6 {
                                return (i-6..i)
                                    .map(|j| parts[j].parse::<f32>().unwrap())
                                    .collect();
                            }
                        }
                    }
                }
            }
        }
        panic!("no cm operator found in stamp Form XObject");
    }

    // -- Tests: get_page_geometry ------------------------------------------

    #[test]
    fn geometry_no_rotation() {
        let pdf = make_test_pdf(612.0, 792.0, None);
        let geo = get_page_geometry(&pdf).unwrap();
        assert_eq!(geo.rotation, 0);
        assert!((geo.raw_width - 612.0).abs() < 0.1);
        assert!((geo.raw_height - 792.0).abs() < 0.1);
        assert!((geo.eff_width - 612.0).abs() < 0.1);
        assert!((geo.eff_height - 792.0).abs() < 0.1);
    }

    #[test]
    fn geometry_rotate_90() {
        let pdf = make_test_pdf(612.0, 792.0, Some(90));
        let geo = get_page_geometry(&pdf).unwrap();
        assert_eq!(geo.rotation, 90);
        // Raw dimensions unchanged
        assert!((geo.raw_width - 612.0).abs() < 0.1);
        assert!((geo.raw_height - 792.0).abs() < 0.1);
        // Effective dimensions swapped
        assert!((geo.eff_width - 792.0).abs() < 0.1);
        assert!((geo.eff_height - 612.0).abs() < 0.1);
    }

    #[test]
    fn geometry_rotate_180() {
        let pdf = make_test_pdf(612.0, 792.0, Some(180));
        let geo = get_page_geometry(&pdf).unwrap();
        assert_eq!(geo.rotation, 180);
        // Effective dimensions same as raw for 180
        assert!((geo.eff_width - 612.0).abs() < 0.1);
        assert!((geo.eff_height - 792.0).abs() < 0.1);
    }

    #[test]
    fn geometry_rotate_270() {
        let pdf = make_test_pdf(612.0, 792.0, Some(270));
        let geo = get_page_geometry(&pdf).unwrap();
        assert_eq!(geo.rotation, 270);
        assert!((geo.eff_width - 792.0).abs() < 0.1);
        assert!((geo.eff_height - 612.0).abs() < 0.1);
    }

    #[test]
    fn get_page_dimensions_returns_effective() {
        let pdf = make_test_pdf(612.0, 792.0, Some(90));
        let (w, h) = get_page_dimensions(&pdf).unwrap();
        // Should return effective (swapped) dimensions
        assert!((w - 792.0).abs() < 0.1);
        assert!((h - 612.0).abs() < 0.1);
    }

    // -- Tests: image_cm_for_rotation --------------------------------------

    #[test]
    fn cm_rotation_0() {
        let [a, b, c, d, e, f] = image_cm_for_rotation(0, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
        assert_eq!([a, b, c, d, e, f], [50.0, 0.0, 0.0, 60.0, 100.0, 200.0]);
    }

    #[test]
    fn cm_rotation_90() {
        // dx=100, dy=200, sw=50, sh=60, W=612, H=792
        let [a, b, c, d, e, f] = image_cm_for_rotation(90, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
        // Expected: [0, sw, -sh, 0, W-dy, dx] = [0, 50, -60, 0, 412, 100]
        assert_eq!([a, b, c, d, e, f], [0.0, 50.0, -60.0, 0.0, 412.0, 100.0]);
    }

    #[test]
    fn cm_rotation_180() {
        let [a, b, c, d, e, f] = image_cm_for_rotation(180, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
        // Expected: [-sw, 0, 0, -sh, W-dx, H-dy] = [-50, 0, 0, -60, 512, 592]
        assert_eq!([a, b, c, d, e, f], [-50.0, 0.0, 0.0, -60.0, 512.0, 592.0]);
    }

    #[test]
    fn cm_rotation_270() {
        let [a, b, c, d, e, f] = image_cm_for_rotation(270, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
        // Expected: [0, -sw, sh, 0, dy, H-dx] = [0, -50, 60, 0, 200, 692]
        assert_eq!([a, b, c, d, e, f], [0.0, -50.0, 60.0, 0.0, 200.0, 692.0]);
    }

    // -- Tests: coord_cm_for_rotation --------------------------------------

    #[test]
    fn coord_cm_rotation_0_is_none() {
        assert!(coord_cm_for_rotation(0, 612.0, 792.0).is_none());
    }

    #[test]
    fn coord_cm_rotation_90() {
        let m = coord_cm_for_rotation(90, 612.0, 792.0).unwrap();
        assert_eq!(m, [0.0, 1.0, -1.0, 0.0, 612.0, 0.0]);
    }

    // -- Tests: stamp_image with rotation -----------------------------------

    #[test]
    fn stamp_image_no_rotation_cm() {
        let pdf = make_test_pdf(612.0, 792.0, None);
        let img = make_red_png();
        let stamped = stamp_image(&pdf, &img, 100.0, 200.0, 50.0, 60.0).unwrap();
        let cm = extract_cm_from_stamped(&stamped);
        assert_eq!(cm, vec![50.0, 0.0, 0.0, 60.0, 100.0, 200.0]);
    }

    #[test]
    fn stamp_image_rotation_90_cm() {
        let pdf = make_test_pdf(612.0, 792.0, Some(90));
        let img = make_red_png();
        // Position in effective (display) coords — effective page is 792 x 612
        let stamped = stamp_image(&pdf, &img, 100.0, 200.0, 50.0, 60.0).unwrap();
        let cm = extract_cm_from_stamped(&stamped);
        // Expected: [0, 50, -60, 0, 612-200, 100] = [0, 50, -60, 0, 412, 100]
        assert_eq!(cm, vec![0.0, 50.0, -60.0, 0.0, 412.0, 100.0]);
    }

    #[test]
    fn stamp_image_rotation_180_cm() {
        let pdf = make_test_pdf(612.0, 792.0, Some(180));
        let img = make_red_png();
        let stamped = stamp_image(&pdf, &img, 100.0, 200.0, 50.0, 60.0).unwrap();
        let cm = extract_cm_from_stamped(&stamped);
        assert_eq!(cm, vec![-50.0, 0.0, 0.0, -60.0, 512.0, 592.0]);
    }

    #[test]
    fn stamp_image_rotation_270_cm() {
        let pdf = make_test_pdf(612.0, 792.0, Some(270));
        let img = make_red_png();
        let stamped = stamp_image(&pdf, &img, 100.0, 200.0, 50.0, 60.0).unwrap();
        let cm = extract_cm_from_stamped(&stamped);
        assert_eq!(cm, vec![0.0, -50.0, 60.0, 0.0, 200.0, 692.0]);
    }

    // -- Tests: stamp_text --------------------------------------------------

    #[test]
    fn stamp_text_no_rotation() {
        let pdf = make_test_pdf(612.0, 792.0, None);
        let result = stamp_text(&pdf, "TEST", 100.0, 200.0, 24.0, "Helvetica", None);
        assert!(result.is_ok());
        // Verify the output is a valid PDF
        let output = result.unwrap();
        let geo = get_page_geometry(&output).unwrap();
        assert_eq!(geo.rotation, 0);
    }

    #[test]
    fn stamp_text_rotation_90() {
        let pdf = make_test_pdf(612.0, 792.0, Some(90));
        let result = stamp_text(&pdf, "TEST", 100.0, 200.0, 24.0, "Helvetica", None);
        assert!(result.is_ok());
    }

    // -- Tests: parse_hex_color -------------------------------------------

    #[test]
    fn parse_hex_color_valid() {
        let (r, g, b) = parse_hex_color("#ff0000").unwrap();
        assert!((r - 1.0).abs() < 0.01);
        assert!(g.abs() < 0.01);
        assert!(b.abs() < 0.01);
    }

    #[test]
    fn parse_hex_color_no_hash() {
        let (r, g, b) = parse_hex_color("00ff00").unwrap();
        assert!(r.abs() < 0.01);
        assert!((g - 1.0).abs() < 0.01);
        assert!(b.abs() < 0.01);
    }

    #[test]
    fn parse_hex_color_invalid() {
        assert!(parse_hex_color("xyz").is_none());
        assert!(parse_hex_color("#ff00").is_none());
    }

    // -- Tests: form BBox uses raw dimensions ------------------------------

    #[test]
    fn form_bbox_uses_raw_dimensions() {
        let pdf = make_test_pdf(612.0, 792.0, Some(90));
        let img = make_red_png();
        let stamped = stamp_image(&pdf, &img, 100.0, 200.0, 50.0, 60.0).unwrap();

        let doc = Document::load_mem(&stamped).unwrap();
        let page_id = doc.page_iter().next().unwrap();
        let page = doc.get_object(page_id).unwrap();
        let page_dict = page.as_dict().unwrap();

        // Find the Stamp Form XObject
        let res = match page_dict.get(b"Resources").unwrap() {
            Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
            Object::Dictionary(d) => d.clone(),
            _ => panic!("unexpected"),
        };
        let xobjects = match res.get(b"XObject").unwrap() {
            Object::Reference(id) => doc.get_object(*id).unwrap().as_dict().unwrap().clone(),
            Object::Dictionary(d) => d.clone(),
            _ => panic!("unexpected"),
        };

        for (name, val) in xobjects.iter() {
            let name_str = std::str::from_utf8(name).unwrap_or("");
            if name_str.starts_with("Stamp") {
                if let Object::Reference(id) = val {
                    if let Ok(Object::Stream(s)) = doc.get_object(*id) {
                        let bbox = s.dict.get(b"BBox").unwrap().as_array().unwrap();
                        // BBox should be [0, 0, raw_width, raw_height] = [0, 0, 612, 792]
                        // NOT [0, 0, 792, 612] (the effective/swapped dimensions)
                        let w = obj_as_f32(&bbox[2]).unwrap();
                        let h = obj_as_f32(&bbox[3]).unwrap();
                        assert!((w - 612.0).abs() < 0.1, "BBox width should be raw 612, got {}", w);
                        assert!((h - 792.0).abs() < 0.1, "BBox height should be raw 792, got {}", h);
                        return;
                    }
                }
            }
        }
        panic!("Stamp Form XObject not found");
    }
}
