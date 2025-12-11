use dioxus::prelude::*;
use wasm_bindgen::JsValue;

pub fn trigger_file_download<'a>(
    filename: &str,
    mime_type: &str,
    data: Vec<u8>,
) -> Result<(), String> {
    let url = generate_blob_url_from_data(data, mime_type)?;

    let script = format!(
        r#"
            const link = document.createElement('a');
            link.href = "{url}";
            link.download = "{filename}";
            document.body.appendChild(link);
            link.click();
            document.body.removeChild(link);
            URL.revokeObjectURL(link.href);
        "#,
        url = url,
        filename = filename
    );

    document::eval(&script);
    Ok(())
}

fn generate_blob_url_from_data(data: Vec<u8>, mime_type: &str) -> Result<String, String> {
    let properties = web_sys::BlobPropertyBag::new();
    properties.set_type(mime_type);

    let js_array: JsValue = web_sys::js_sys::Uint8Array::from(&data[..]).into();
    let buffer: web_sys::js_sys::Array = IntoIterator::into_iter([js_array]).collect();
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&buffer, &properties)
        .map_err(|e| format!("Failed to create Blob: {:?}", e))?;

    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("Failed to create Object URL: {:?}", e))?;

    Ok(url)
}
