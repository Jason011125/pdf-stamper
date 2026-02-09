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

/// Extract page dimensions (width, height) in points from the first page.
pub fn get_page_dimensions(pdf_bytes: &[u8]) -> Result<(f32, f32), PdfError> {
    let doc = Document::load_mem(pdf_bytes)
        .map_err(|e| PdfError::LoadError(e.to_string()))?;

    let page_id = doc
        .page_iter()
        .next()
        .ok_or_else(|| PdfError::LoadError("PDF has no pages".into()))?;

    let page = doc
        .get_object(page_id)
        .map_err(|e| PdfError::LoadError(e.to_string()))?;

    let media_box = page
        .as_dict()
        .ok()
        .and_then(|d| d.get(b"MediaBox").ok())
        .and_then(|o| o.as_array().ok())
        .ok_or_else(|| PdfError::LoadError("cannot read MediaBox".into()))?;

    if media_box.len() >= 4 {
        let width = media_box[2]
            .as_float()
            .or_else(|_| media_box[2].as_i64().map(|i| i as f32))
            .unwrap_or(612.0);
        let height = media_box[3]
            .as_float()
            .or_else(|_| media_box[3].as_i64().map(|i| i as f32))
            .unwrap_or(792.0);
        Ok((width, height))
    } else {
        Ok((612.0, 792.0))
    }
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

/// Overlay an image stamp on the first page of a PDF.
/// Uses a self-contained Form XObject so we only need to register one name
/// in the page's Resources, and the image lives inside the Form's own Resources.
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

    // Create image XObject manually (lopdf's image_from mishandles RGBA)
    let img_id = create_image_xobject(&mut doc, image_bytes)?;

    // Build a Form XObject that draws the image, carrying its own Resources
    let img_ref_name = b"Img0";
    let form_ops = vec![
        Operation::new("q", vec![]),
        Operation::new(
            "cm",
            vec![
                width.into(), 0.into(), 0.into(), height.into(), x.into(), y.into(),
            ],
        ),
        Operation::new("Do", vec![Name(img_ref_name.to_vec())]),
        Operation::new("Q", vec![]),
    ];
    let form_content = Content { operations: form_ops }
        .encode()
        .map_err(|e| PdfError::StampError(e.to_string()))?;

    let (page_w, page_h) = get_page_dimensions(pdf_bytes)?;

    let mut xobjects = Dictionary::new();
    xobjects.set(img_ref_name.as_slice(), Object::Reference(img_id));
    let mut resources = Dictionary::new();
    resources.set("XObject", Object::Dictionary(xobjects));

    let mut form_dict = Dictionary::new();
    form_dict.set("Type", Name(b"XObject".to_vec()));
    form_dict.set("Subtype", Name(b"Form".to_vec()));
    form_dict.set(
        "BBox",
        Object::Array(vec![0.0f32.into(), 0.0f32.into(), page_w.into(), page_h.into()]),
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

    let (page_w, page_h) = get_page_dimensions(pdf_bytes)?;

    let mut fonts = Dictionary::new();
    fonts.set(font_ref_name.as_slice(), Object::Reference(font_id));
    let mut resources = Dictionary::new();
    resources.set("Font", Object::Dictionary(fonts));

    let mut form_dict = Dictionary::new();
    form_dict.set("Type", Name(b"XObject".to_vec()));
    form_dict.set("Subtype", Name(b"Form".to_vec()));
    form_dict.set(
        "BBox",
        Object::Array(vec![0.0f32.into(), 0.0f32.into(), page_w.into(), page_h.into()]),
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

    fn describe_obj(obj: &Object) -> String {
        match obj {
            Object::Reference(id) => format!("Reference({} {})", id.0, id.1),
            Object::Dictionary(d) => format!("Dictionary(len={})", d.len()),
            Object::Array(a) => format!("Array(len={})", a.len()),
            Object::Stream(_) => "Stream(...)".into(),
            other => format!("{:?}", other),
        }
    }

    fn print_dict(dict: &Dictionary, indent: &str) {
        for (key, val) in dict.iter() {
            println!("{}{} = {}",
                indent,
                std::str::from_utf8(key).unwrap_or("?"),
                describe_obj(val),
            );
        }
    }

    #[test]
    fn diagnose_stamp_image() {
        let pdf_bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../testing_files/test.pdf"
        ))
        .expect("failed to read test.pdf");
        let img_bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../testing_files/stamp.png"
        ))
        .expect("failed to read stamp.png");

        println!("=== INPUT PDF ===");
        println!("PDF size: {} bytes", pdf_bytes.len());
        println!("Image size: {} bytes", img_bytes.len());

        let doc = Document::load_mem(&pdf_bytes).expect("load PDF");
        let page_id = doc.page_iter().next().expect("no pages");
        println!("Page ID: {:?}", page_id);

        let page = doc.get_object(page_id).expect("get page");
        let page_dict = page.as_dict().expect("page as dict");
        println!("\n=== PAGE DICT ===");
        print_dict(page_dict, "  ");

        // Check Resources
        println!("\n=== RESOURCES ===");
        match page_dict.get(b"Resources") {
            Ok(Object::Reference(id)) => {
                println!("Resources is INDIRECT ref: {:?}", id);
                if let Ok(res_obj) = doc.get_object(*id) {
                    if let Ok(res_dict) = res_obj.as_dict() {
                        print_dict(res_dict, "  ");
                        // Check XObject sub-dict
                        if let Ok(xo) = res_dict.get(b"XObject") {
                            println!("  XObject detail: {}", describe_obj(xo));
                        }
                    }
                }
            }
            Ok(Object::Dictionary(d)) => {
                println!("Resources is INLINE:");
                print_dict(d, "  ");
            }
            Ok(other) => println!("Resources unexpected: {}", describe_obj(other)),
            Err(_) => println!("Resources NOT FOUND (inherited from parent)"),
        }

        // Check Contents
        println!("\n=== CONTENTS ===");
        match page_dict.get(b"Contents") {
            Ok(Object::Reference(id)) => println!("Single ref: {:?}", id),
            Ok(Object::Array(arr)) => {
                println!("Array with {} entries:", arr.len());
                for (i, item) in arr.iter().enumerate() {
                    println!("  [{}] {}", i, describe_obj(item));
                }
            }
            Ok(other) => println!("{}", describe_obj(other)),
            Err(_) => println!("NOT FOUND"),
        }

        // Stamp it
        println!("\n=== STAMPING (100,100) 200x200 ===");
        let result = stamp_image(&pdf_bytes, &img_bytes, 100.0, 100.0, 200.0, 200.0);
        match &result {
            Ok(output) => {
                println!("OK — output {} bytes", output.len());

                let out_doc = Document::load_mem(output).expect("reload");
                let out_page_id = out_doc.page_iter().next().unwrap();
                let out_page = out_doc.get_object(out_page_id).unwrap();
                let out_dict = out_page.as_dict().unwrap();

                println!("\n=== OUTPUT PAGE ===");
                print_dict(out_dict, "  ");

                // Resources/XObject in output
                println!("\n=== OUTPUT RESOURCES ===");
                match out_dict.get(b"Resources") {
                    Ok(Object::Reference(res_id)) => {
                        println!("Indirect ref: {:?}", res_id);
                        if let Ok(r) = out_doc.get_object(*res_id) {
                            if let Ok(rd) = r.as_dict() {
                                print_dict(rd, "  ");
                                if let Ok(Object::Reference(xo_id)) = rd.get(b"XObject") {
                                    println!("  -> XObject deref {:?}:", xo_id);
                                    if let Ok(xo) = out_doc.get_object(*xo_id) {
                                        if let Ok(xd) = xo.as_dict() {
                                            print_dict(xd, "      ");
                                        }
                                    }
                                } else if let Ok(Object::Dictionary(xd)) = rd.get(b"XObject") {
                                    println!("  -> XObject inline:");
                                    print_dict(xd, "      ");
                                }
                            }
                        }
                    }
                    Ok(Object::Dictionary(d)) => {
                        println!("Inline:");
                        print_dict(d, "  ");
                    }
                    Ok(_) => println!("unexpected type"),
                    Err(_) => println!("NOT FOUND on output page!"),
                }

                // Contents in output
                println!("\n=== OUTPUT CONTENTS ===");
                match out_dict.get(b"Contents") {
                    Ok(Object::Array(arr)) => {
                        println!("Array with {} entries", arr.len());
                        for (i, item) in arr.iter().enumerate() {
                            if let Object::Reference(id) = item {
                                if let Ok(Object::Stream(s)) = out_doc.get_object(*id) {
                                    let mut sc = s.clone();
                                    let _ = sc.decompress();
                                    let txt = String::from_utf8_lossy(&sc.content);
                                    println!("  [{}] ref {:?}: {}", i, id,
                                        if txt.len() > 200 {
                                            format!("{}... ({} bytes)", &txt[..200], txt.len())
                                        } else {
                                            txt.to_string()
                                        }
                                    );
                                }
                            }
                        }
                    }
                    Ok(Object::Reference(id)) => println!("Still single ref: {:?}", id),
                    _ => println!("unexpected"),
                }

                // Inspect the Form XObject itself
                println!("\n=== FORM XOBJECT (Stamp2036) ===");
                // Find it
                if let Ok(Object::Reference(res_id)) = out_dict.get(b"Resources") {
                    if let Ok(res) = out_doc.get_object(*res_id) {
                        if let Ok(rd) = res.as_dict() {
                            if let Ok(xobj_entry) = rd.get(b"XObject") {
                                if let Ok(xd) = xobj_entry.as_dict() {
                                    for (name, val) in xd.iter() {
                                        if let Object::Reference(id) = val {
                                            if let Ok(Object::Stream(s)) = out_doc.get_object(*id) {
                                                println!("  {} -> ref {:?}", std::str::from_utf8(name).unwrap_or("?"), id);
                                                println!("    Dict:");
                                                print_dict(&s.dict, "      ");
                                                let mut sc = s.clone();
                                                let _ = sc.decompress();
                                                let txt = String::from_utf8_lossy(&sc.content);
                                                if txt.len() < 500 {
                                                    println!("    Content: {:?}", txt);
                                                } else {
                                                    println!("    Content: {} bytes (not a form)", sc.content.len());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Inspect the stamp IMAGE object (inside Form's Resources)
                println!("\n=== STAMP IMAGE OBJECT ===");
                if let Ok(Object::Stream(form_stream)) = out_doc.get_object((2036, 0)) {
                    if let Ok(form_res) = form_stream.dict.get(b"Resources") {
                        if let Ok(form_res_dict) = form_res.as_dict() {
                            println!("  Form Resources:");
                            print_dict(form_res_dict, "    ");
                            if let Ok(xobj) = form_res_dict.get(b"XObject") {
                                if let Ok(xobj_dict) = xobj.as_dict() {
                                    for (name, val) in xobj_dict.iter() {
                                        println!("    {} = {}", std::str::from_utf8(name).unwrap_or("?"), describe_obj(val));
                                        if let Object::Reference(img_ref) = val {
                                            if let Ok(Object::Stream(img_s)) = out_doc.get_object(*img_ref) {
                                                println!("    -> Image dict:");
                                                print_dict(&img_s.dict, "        ");
                                                println!("    -> Image data: {} bytes", img_s.content.len());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Render input and output to compare visually
                println!("\n=== RENDER COMPARISON ===");
                let input_png = render_page_to_png(&pdf_bytes, 800).expect("render input");
                let output_png = render_page_to_png(output, 800).expect("render output");
                println!("Input PNG: {} bytes", input_png.len());
                println!("Output PNG: {} bytes", output_png.len());
                println!("PNGs differ: {}", input_png != output_png);

                let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../testing_files/");
                std::fs::write(format!("{base}test-stamped-debug.pdf"), output).expect("write pdf");
                std::fs::write(format!("{base}render-input.png"), &input_png).expect("write input png");
                std::fs::write(format!("{base}render-output.png"), &output_png).expect("write output png");
                println!("Wrote debug files to testing_files/");
            }
            Err(e) => println!("FAILED: {}", e),
        }
    }
}
