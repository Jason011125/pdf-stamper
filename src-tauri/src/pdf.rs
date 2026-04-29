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

/// Compute the `cm` matrix [a, b, c, d, e, f] for placing an image stamp at
/// display position `(dx, dy)` with display size `(sw, sh)` on a page whose
/// raw MediaBox is `(raw_w, raw_h)` and whose `/Rotate` is `rotation` degrees CW.
///
/// PDF Image XObject convention (PDF 1.7 §8.9.5): the source image's first
/// sample (top-left) maps to unit-square (0, 1) and the last (bottom-right)
/// to (1, 0). The minimal "upright at (x, y) sized (w, h)" matrix is therefore
/// `[w, 0, 0, h, x, y]` — no Y flip.
///
/// For rotated pages, the viewer applies `T_rotate` (raw → display) at render
/// time; we compose `M_display→raw · M_image→display` so that, after the viewer
/// rotates by `rotation`° CW, the stamp lands upright at display `(dx, dy)`.
///
/// **Do not modify without re-verifying via the property-based tests** in this
/// module (see `assert_stamp_corners_at_display`). The full derivation lives
/// in `.claude/CLAUDE.md` under "PDF Image Stamp `cm` Matrix Recipe".
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
        _   => [sw,  0.0, 0.0,  sh, dx, dy],
    }
}

/// Compute the coordinate-space `cm` matrix used for text stamps. After
/// applying this `cm`, subsequent operations (`Td x y`, `Tj`, …) act in a
/// coordinate system whose axes and origin coincide with the **display**
/// (post-`/Rotate`) page, so the caller can use display-space `(x, y)` and
/// the text renders upright at that display position.
///
/// Returns `None` for `/Rotate=0` (no transform needed).
///
/// The previous implementation derived this as the rotation that maps raw
/// to display, which is the inverse of what `cm` actually needs (`cm` maps
/// the *new* coordinate system to the *outer* one — here, display → raw).
/// The 90° and 270° branches were swapped because raw→display and its
/// inverse coincide only at /Rotate=180.
fn coord_cm_for_rotation(rotation: u32, raw_w: f32, raw_h: f32) -> Option<[f32; 6]> {
    // Display → raw, derived from the inverse of T_rotate (raw → display):
    //   /Rotate= 90 CW: T_rotate (u, v)_raw → (v, raw_w − u)_display
    //                   inverse  (x, y)_display → (raw_w − y, x)_raw
    //                   cm = [0, 1, −1, 0, raw_w, 0]
    //   /Rotate=180:    T_rotate (u, v)_raw → (raw_w − u, raw_h − v)_display
    //                   inverse  (x, y)_display → (raw_w − x, raw_h − y)_raw
    //                   cm = [−1, 0, 0, −1, raw_w, raw_h]
    //   /Rotate=270 CW: T_rotate (u, v)_raw → (raw_h − v, u)_display
    //                   inverse  (x, y)_display → (y, raw_h − x)_raw
    //                   cm = [0, −1, 1, 0, 0, raw_h]
    //
    // Verified by `text_coord_cm_maps_display_back_to_display` in tests.
    match rotation {
        90  => Some([ 0.0,  1.0, -1.0,  0.0, raw_w, 0.0  ]),
        180 => Some([-1.0,  0.0,  0.0, -1.0, raw_w, raw_h]),
        270 => Some([ 0.0, -1.0,  1.0,  0.0, 0.0,   raw_h]),
        _   => None,
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

/// Append a stamp content stream to the page's Contents, isolating it from
/// the existing content's graphics state.
///
/// Many PDF generators emit a top-level `cm` at the start of the content
/// stream (typically `[s, 0, 0, -s, 0, page_height]` to use a top-down
/// "screen-like" coordinate system) that is **not** balanced by a `q ... Q`
/// pair. Such PDFs leave the CTM non-identity at end-of-stream. If we just
/// append our stamp content, our `cm` composes with the leftover CTM and
/// our stamp ends up scaled / flipped.
///
/// To stay generic, we wrap the original Contents in `q ... Q` and put our
/// stamp stream after the closing `Q`. The `Q` rolls back any CTM (and
/// other graphics-state) changes the original content made, so our stamp
/// always runs in the page's initial graphics state.
fn append_content_stream(
    doc: &mut Document,
    page_id: lopdf::ObjectId,
    stream_id: lopdf::ObjectId,
) -> Result<(), PdfError> {
    let q_open_id = doc.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let q_close_id = doc.add_object(Stream::new(Dictionary::new(), b"Q\n".to_vec()));

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
                    Object::Reference(q_open_id),
                    Object::Reference(existing_id),
                    Object::Reference(q_close_id),
                    Object::Reference(stream_id),
                ]),
            );
        }
        Ok(Object::Array(arr)) => {
            let mut new_arr = vec![Object::Reference(q_open_id)];
            new_arr.extend(arr.clone());
            new_arr.push(Object::Reference(q_close_id));
            new_arr.push(Object::Reference(stream_id));
            page_dict.set("Contents", Object::Array(new_arr));
        }
        _ => {
            // No existing Contents: nothing to wrap, just set ours.
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

    // -- Tests: image_cm_for_rotation (property-based) ---------------------
    //
    // We do NOT compare matrix values against hand-computed expressions —
    // that would just encode the same bug pattern that shipped in c6a9a80.
    // Instead, for each unit-square corner of the image, verify that
    // (image → raw via cm) + (raw → display via T_rotate) lands at the
    // expected display-space corner of the user-picked stamp rectangle.

    fn apply_cm(cm: [f32; 6], (u, v): (f32, f32)) -> (f32, f32) {
        (cm[0] * u + cm[2] * v + cm[4], cm[1] * u + cm[3] * v + cm[5])
    }

    /// Apply the viewer's raw → display rotation (CW by `rotation` degrees).
    fn apply_t_rotate(rotation: u32, (rx, ry): (f32, f32), raw_w: f32, raw_h: f32) -> (f32, f32) {
        match rotation {
            90  => (ry, raw_w - rx),
            180 => (raw_w - rx, raw_h - ry),
            270 => (raw_h - ry, rx),
            _   => (rx, ry),
        }
    }

    /// For each unit-square corner of the image, assert that after applying
    /// `cm` (image → raw) and then `T_rotate` (raw → display), the corner
    /// lands at the matching corner of the user-picked stamp rectangle.
    fn assert_stamp_corners_at_display(
        rotation: u32,
        cm: [f32; 6],
        dx: f32, dy: f32,
        w: f32, h: f32,
        raw_w: f32, raw_h: f32,
    ) {
        let cases: [((f32, f32), (f32, f32)); 4] = [
            ((0.0, 0.0), (dx,     dy)),       // image bottom-left → stamp BL
            ((1.0, 0.0), (dx + w, dy)),       // image bottom-right → stamp BR
            ((0.0, 1.0), (dx,     dy + h)),   // image top-left → stamp TL
            ((1.0, 1.0), (dx + w, dy + h)),   // image top-right → stamp TR
        ];
        for (unit, expected) in cases {
            let raw = apply_cm(cm, unit);
            let disp = apply_t_rotate(rotation, raw, raw_w, raw_h);
            assert!(
                (disp.0 - expected.0).abs() < 0.01 && (disp.1 - expected.1).abs() < 0.01,
                "rotation={}, unit={:?}: stamp corner landed at display={:?}, expected={:?}",
                rotation, unit, disp, expected,
            );
        }
    }

    #[test]
    fn image_stamp_corners_no_rotation() {
        let cm = image_cm_for_rotation(0, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
        assert_stamp_corners_at_display(0, cm, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
    }

    #[test]
    fn image_stamp_corners_rotation_90() {
        let cm = image_cm_for_rotation(90, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
        assert_stamp_corners_at_display(90, cm, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
    }

    #[test]
    fn image_stamp_corners_rotation_180() {
        let cm = image_cm_for_rotation(180, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
        assert_stamp_corners_at_display(180, cm, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
    }

    #[test]
    fn image_stamp_corners_rotation_270() {
        let cm = image_cm_for_rotation(270, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
        assert_stamp_corners_at_display(270, cm, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
    }

    #[test]
    fn image_stamp_corners_landscape_page_rotation_90() {
        // Different page aspect ratio + non-trivial offset to catch matrices
        // that happen to be correct for square-ish pages but wrong elsewhere.
        let cm = image_cm_for_rotation(90, 50.0, 30.0, 200.0, 80.0, 1000.0, 400.0);
        assert_stamp_corners_at_display(90, cm, 50.0, 30.0, 200.0, 80.0, 1000.0, 400.0);
    }

    // -- Tests: coord_cm_for_rotation (property-based) ---------------------

    #[test]
    fn coord_cm_rotation_0_is_none() {
        assert!(coord_cm_for_rotation(0, 612.0, 792.0).is_none());
    }

    /// Verify that for every test display point, applying `cm` (display→raw)
    /// followed by `T_rotate` (raw→display) returns the original display
    /// point. This pins down both the linear part and the translation of the
    /// affine map without re-encoding the same matrix expression in the test.
    fn assert_coord_cm_roundtrips(rotation: u32, raw_w: f32, raw_h: f32) {
        let cm = coord_cm_for_rotation(rotation, raw_w, raw_h)
            .expect("non-zero rotation should produce a cm");
        let display_points: [(f32, f32); 5] = [
            (0.0, 0.0),
            (100.0, 200.0),
            (50.0, 30.0),
            (raw_w / 2.0, raw_h / 2.0),
            (1.0, 0.0), // probes orientation: must stay on the +x axis after roundtrip
        ];
        for &(dx, dy) in &display_points {
            let raw = apply_cm(cm, (dx, dy));
            let back = apply_t_rotate(rotation, raw, raw_w, raw_h);
            assert!(
                (back.0 - dx).abs() < 0.01 && (back.1 - dy).abs() < 0.01,
                "rotation={}, display=({}, {}): cm+T_rotate landed at {:?}",
                rotation, dx, dy, back,
            );
        }
    }

    #[test]
    fn coord_cm_rotation_90_roundtrips() {
        assert_coord_cm_roundtrips(90, 612.0, 792.0);
    }

    #[test]
    fn coord_cm_rotation_180_roundtrips() {
        assert_coord_cm_roundtrips(180, 612.0, 792.0);
    }

    #[test]
    fn coord_cm_rotation_270_roundtrips() {
        assert_coord_cm_roundtrips(270, 612.0, 792.0);
    }

    #[test]
    fn coord_cm_rotation_landscape_page() {
        // Non-square page exposes any matrix that mixes raw_w and raw_h.
        assert_coord_cm_roundtrips(90, 1000.0, 400.0);
        assert_coord_cm_roundtrips(270, 1000.0, 400.0);
    }

    // -- Tests: stamp_image full pipeline (property-based) -----------------
    //
    // End-to-end check: build a PDF, run stamp_image, extract the cm that
    // the stamping code emitted into the Form XObject, and verify the same
    // post-rotation corner property as the unit tests above. This catches
    // any regression in either image_cm_for_rotation or how stamp_image
    // wires it into the Form XObject content stream.

    fn cm_vec_to_array(v: &[f32]) -> [f32; 6] {
        [v[0], v[1], v[2], v[3], v[4], v[5]]
    }

    #[test]
    fn stamp_image_lands_at_display_no_rotation() {
        let pdf = make_test_pdf(612.0, 792.0, None);
        let img = make_red_png();
        let stamped = stamp_image(&pdf, &img, 100.0, 200.0, 50.0, 60.0).unwrap();
        let cm = cm_vec_to_array(&extract_cm_from_stamped(&stamped));
        assert_stamp_corners_at_display(0, cm, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
    }

    #[test]
    fn stamp_image_lands_at_display_rotation_90() {
        let pdf = make_test_pdf(612.0, 792.0, Some(90));
        let img = make_red_png();
        let stamped = stamp_image(&pdf, &img, 100.0, 200.0, 50.0, 60.0).unwrap();
        let cm = cm_vec_to_array(&extract_cm_from_stamped(&stamped));
        assert_stamp_corners_at_display(90, cm, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
    }

    #[test]
    fn stamp_image_lands_at_display_rotation_180() {
        let pdf = make_test_pdf(612.0, 792.0, Some(180));
        let img = make_red_png();
        let stamped = stamp_image(&pdf, &img, 100.0, 200.0, 50.0, 60.0).unwrap();
        let cm = cm_vec_to_array(&extract_cm_from_stamped(&stamped));
        assert_stamp_corners_at_display(180, cm, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
    }

    #[test]
    fn stamp_image_lands_at_display_rotation_270() {
        let pdf = make_test_pdf(612.0, 792.0, Some(270));
        let img = make_red_png();
        let stamped = stamp_image(&pdf, &img, 100.0, 200.0, 50.0, 60.0).unwrap();
        let cm = cm_vec_to_array(&extract_cm_from_stamped(&stamped));
        assert_stamp_corners_at_display(270, cm, 100.0, 200.0, 50.0, 60.0, 612.0, 792.0);
    }

    // -- Regression: stamp survives a polluted CTM in existing Contents ----
    //
    // Many real-world PDFs (e.g. exports from web-based tools) start their
    // content stream with a top-level `cm` that is NOT inside a q/Q pair —
    // typically `[s, 0, 0, -s, 0, page_height]` to use a top-down coordinate
    // system. End-of-stream CTM is therefore non-identity. We must wrap the
    // existing Contents in `q ... Q` so our stamp runs in clean state.

    fn make_polluted_ctm_pdf(width: f32, height: f32, ctm: [f32; 6]) -> Vec<u8> {
        let mut doc = Document::new();

        // Single content stream that begins with the polluted cm and never
        // restores it. This mimics what real-world generators emit.
        let body = format!(
            "{} {} {} {} {} {} cm\n",
            ctm[0], ctm[1], ctm[2], ctm[3], ctm[4], ctm[5]
        );
        let content = Stream::new(Dictionary::new(), body.into_bytes());
        let content_id = doc.add_object(content);

        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Name(b"Page".to_vec()));
        page_dict.set(
            "MediaBox",
            Object::Array(vec![
                0.0f32.into(), 0.0f32.into(), width.into(), height.into(),
            ]),
        );
        page_dict.set("Contents", Object::Reference(content_id));
        let page_id = doc.add_object(Object::Dictionary(page_dict));

        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", Name(b"Pages".to_vec()));
        pages_dict.set("Count", Object::Integer(1));
        pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
        let pages_id = doc.add_object(Object::Dictionary(pages_dict));

        let page = doc.get_object_mut(page_id).unwrap();
        page.as_dict_mut().unwrap().set("Parent", Object::Reference(pages_id));

        let mut catalog = Dictionary::new();
        catalog.set("Type", Name(b"Catalog".to_vec()));
        catalog.set("Pages", Object::Reference(pages_id));
        let catalog_id = doc.add_object(Object::Dictionary(catalog));
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }

    fn make_solid_red_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |_, _| image::Rgb([255, 0, 0]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
        bytes
    }

    /// Render a stamped PDF via pdfium and return the bbox (in render-image
    /// pixels, y-from-top) of pure red pixels — i.e. where the stamp lies.
    fn red_bbox_in_render(stamped: &[u8], target_width: u16) -> ((u32, u32), (u32, u32)) {
        let png = render_page_to_png(stamped, target_width).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        let rgba = img.to_rgba8();
        let (mut x_min, mut x_max) = (u32::MAX, 0u32);
        let (mut y_min, mut y_max) = (u32::MAX, 0u32);
        for (x, y, p) in rgba.enumerate_pixels() {
            if p[0] > 200 && p[1] < 80 && p[2] < 80 {
                x_min = x_min.min(x); x_max = x_max.max(x);
                y_min = y_min.min(y); y_max = y_max.max(y);
            }
        }
        assert!(x_min != u32::MAX, "no red pixels found in rendered output");
        ((x_min, x_max), (y_min, y_max))
    }

    #[test]
    fn stamp_image_survives_polluted_ctm_in_existing_contents() {
        // Polluted CTM: scale 0.5, Y-flip, translate to keep content visible.
        // This is the exact pattern observed in real exports from web tools.
        let polluted = [0.5_f32, 0.0, 0.0, -0.5, 0.0, 800.0];
        let pdf = make_polluted_ctm_pdf(600.0, 800.0, polluted);
        let img = make_solid_red_png(50, 50);

        let (dx, dy, w, h) = (200.0_f32, 300.0_f32, 100.0_f32, 100.0_f32);
        let stamped = stamp_image(&pdf, &img, dx, dy, w, h).unwrap();

        let target_w = 600u16;
        let ((rx_min, rx_max), (ry_min, ry_max)) = red_bbox_in_render(&stamped, target_w);

        // Expected bbox in render coords:
        //   render scale = target_w / page_w
        //   x = dx*scale .. (dx+w)*scale
        //   y_from_top = (page_h - dy - h)*scale .. (page_h - dy)*scale
        let scale = target_w as f32 / 600.0;
        let render_h = 800.0 * scale;
        let exp_x_min = dx * scale;
        let exp_x_max = (dx + w) * scale;
        let exp_y_min = render_h - (dy + h) * scale;
        let exp_y_max = render_h - dy * scale;

        // Allow small tolerance for anti-aliasing at the edges.
        let tol = 4.0_f32;
        let close = |actual: u32, expected: f32| (actual as f32 - expected).abs() <= tol;
        assert!(close(rx_min, exp_x_min) && close(rx_max, exp_x_max),
            "x bbox: got {}..{}, expected {:.0}..{:.0}", rx_min, rx_max, exp_x_min, exp_x_max);
        assert!(close(ry_min, exp_y_min) && close(ry_max, exp_y_max),
            "y bbox: got {}..{}, expected {:.0}..{:.0}", ry_min, ry_max, exp_y_min, exp_y_max);
    }

    // -- Diagnostic: stamp a real PDF, render via pdfium, locate stamp -----
    //
    // Run with:  DIAGNOSE_PDF=/path/to/file.pdf cargo test --lib diagnose_real_pdf -- --ignored --nocapture

    #[test]
    #[ignore]
    fn diagnose_real_pdf() {
        let pdf_path = std::env::var("DIAGNOSE_PDF")
            .expect("set DIAGNOSE_PDF=/path/to/file.pdf");
        let pdf_bytes = std::fs::read(&pdf_path).expect("read source pdf");

        // -- Inspect existing Contents stream(s) for q/Q balance and trailing CTM
        {
            let doc = Document::load_mem(&pdf_bytes).unwrap();
            let pid = doc.page_iter().next().unwrap();
            let page = doc.get_object(pid).unwrap().as_dict().unwrap();
            let stream_ids: Vec<lopdf::ObjectId> = match page.get(b"Contents").unwrap() {
                Object::Reference(id) => vec![*id],
                Object::Array(arr) => arr.iter().filter_map(|o|
                    if let Object::Reference(i) = o { Some(*i) } else { None }).collect(),
                _ => vec![],
            };
            eprintln!("\nexisting Contents = {} stream(s)", stream_ids.len());
            let mut q_count = 0i64;
            let mut last_cm: Option<[f32; 6]> = None;
            for sid in &stream_ids {
                if let Ok(Object::Stream(s)) = doc.get_object(*sid) {
                    let mut sc = s.clone();
                    let _ = sc.decompress();
                    let body = String::from_utf8_lossy(&sc.content).to_string();
                    let toks: Vec<&str> = body.split_whitespace().collect();
                    for (i, t) in toks.iter().enumerate() {
                        match *t {
                            "q" => q_count += 1,
                            "Q" => q_count -= 1,
                            "cm" if i >= 6 => {
                                let mut m = [0.0_f32; 6];
                                let mut ok = true;
                                for j in 0..6 {
                                    match toks[i - 6 + j].parse::<f32>() {
                                        Ok(v) => m[j] = v,
                                        Err(_) => { ok = false; break; }
                                    }
                                }
                                if ok { last_cm = Some(m); }
                            }
                            _ => {}
                        }
                    }
                }
            }
            eprintln!("  q/Q balance after all streams: {} (expected 0; positive = unrestored q on stack)", q_count);
            if let Some(m) = last_cm {
                eprintln!("  last cm seen in any stream: [{:.4}, {:.4}, {:.4}, {:.4}, {:.4}, {:.4}]",
                    m[0], m[1], m[2], m[3], m[4], m[5]);
            }

            // Dump the existing content streams to /tmp for inspection.
            let mut all_body = String::new();
            for sid in &stream_ids {
                if let Ok(Object::Stream(s)) = doc.get_object(*sid) {
                    let mut sc = s.clone();
                    let _ = sc.decompress();
                    all_body.push_str(&String::from_utf8_lossy(&sc.content));
                    all_body.push('\n');
                }
            }
            std::fs::write("/tmp/existing-contents.txt", &all_body).unwrap();
            eprintln!("  full existing content stream dumped to /tmp/existing-contents.txt ({} bytes)", all_body.len());
        }

        // Also dump the stamped PDF's last content stream (the one we appended)
        // so we can compare it with what the existing PDF's state expects.
        {
            // (continued below — first need stamped, see end of test for dump)
        }

        // 50×50 solid red, will be drawn at 100×100 — easy to spot in render.
        let red = image::RgbImage::from_fn(50, 50, |_, _| image::Rgb([255, 0, 0]));
        let mut img_bytes = Vec::new();
        red.write_to(&mut std::io::Cursor::new(&mut img_bytes), image::ImageFormat::Png).unwrap();

        let (dx, dy, w, h) = (446.8_f32, 339.0_f32, 100.0_f32, 100.0_f32);
        let geo = get_page_geometry(&pdf_bytes).unwrap();
        eprintln!("\nsource PDF:");
        eprintln!("  raw  dims = {} x {}", geo.raw_width, geo.raw_height);
        eprintln!("  /Rotate   = {}", geo.rotation);
        eprintln!("  eff  dims = {} x {}", geo.eff_width, geo.eff_height);
        eprintln!("stamp params:");
        eprintln!("  display position (bottom-left of stamp) = ({}, {})", dx, dy);
        eprintln!("  display size                            = {} x {}", w, h);

        let stamped = stamp_image(&pdf_bytes, &img_bytes, dx, dy, w, h).unwrap();
        let out_pdf = "/tmp/stamped-diagnostic.pdf";
        std::fs::write(out_pdf, &stamped).unwrap();
        eprintln!("\nstamped PDF written to {} ({} bytes)", out_pdf, stamped.len());

        let cm = extract_cm_from_stamped(&stamped);
        eprintln!("emitted cm                  = [{}]",
            cm.iter().map(|v| format!("{:.2}", v)).collect::<Vec<_>>().join(", "));

        // Where do the four image corners actually land in the stamped page?
        let cm_arr = [cm[0], cm[1], cm[2], cm[3], cm[4], cm[5]];
        for (label, (u, v)) in [
            ("image bottom-left  unit(0,0)", (0.0_f32, 0.0_f32)),
            ("image bottom-right unit(1,0)", (1.0, 0.0)),
            ("image top-left     unit(0,1)", (0.0, 1.0)),
            ("image top-right    unit(1,1)", (1.0, 1.0)),
        ] {
            let raw_x = cm_arr[0] * u + cm_arr[2] * v + cm_arr[4];
            let raw_y = cm_arr[1] * u + cm_arr[3] * v + cm_arr[5];
            eprintln!("  {} -> raw ({:.2}, {:.2})", label, raw_x, raw_y);
        }

        // Render the result and locate the red stamp
        let png = render_page_to_png(&stamped, 800).unwrap();
        let out_png = "/tmp/stamped-diagnostic.png";
        std::fs::write(out_png, &png).unwrap();

        let img = image::load_from_memory(&png).unwrap();
        let (rw, rh) = (img.width(), img.height());
        let rgba = img.to_rgba8();
        let mut x_min = u32::MAX; let mut x_max = 0u32;
        let mut y_min = u32::MAX; let mut y_max = 0u32;
        let mut count = 0u32;
        for (x, y, p) in rgba.enumerate_pixels() {
            if p[0] > 200 && p[1] < 80 && p[2] < 80 {
                if x < x_min { x_min = x; }
                if x > x_max { x_max = x; }
                if y < y_min { y_min = y; }
                if y > y_max { y_max = y; }
                count += 1;
            }
        }
        eprintln!("\nrendered preview image ({}x{}) saved to {}", rw, rh, out_png);
        if count == 0 {
            panic!("no red stamp pixels found in rendered output");
        }
        eprintln!("red stamp pixel bbox in rendered image:");
        eprintln!("  x = {}..{} ({:.1}% .. {:.1}% of width)",
            x_min, x_max, 100.0 * x_min as f32 / rw as f32, 100.0 * x_max as f32 / rw as f32);
        eprintln!("  y = {}..{} ({:.1}% .. {:.1}% from TOP of image)",
            y_min, y_max, 100.0 * y_min as f32 / rh as f32, 100.0 * y_max as f32 / rh as f32);

        // Where SHOULD the red stamp land in the rendered image?
        // Display bbox = (dx, dy) to (dx+w, dy+h) in PDF points (y from bottom).
        // pdfium renders eff_width:eff_height -> rw:rh at scale rw/eff_width.
        let scale = rw as f32 / geo.eff_width;
        let exp_x_min = dx * scale;
        let exp_x_max = (dx + w) * scale;
        let exp_y_top_in_render = (geo.eff_height - (dy + h)) * scale;     // top edge of stamp in render (y from top of image)
        let exp_y_bot_in_render = (geo.eff_height - dy) * scale;
        eprintln!("expected red bbox if cm is correct (PDF→render):");
        eprintln!("  x = {:.1}..{:.1}", exp_x_min, exp_x_max);
        eprintln!("  y = {:.1}..{:.1} (from top of rendered image)", exp_y_top_in_render, exp_y_bot_in_render);
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
