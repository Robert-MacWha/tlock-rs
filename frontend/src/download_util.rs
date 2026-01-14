use wasm_bindgen::JsCast;
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url, js_sys};

pub fn download_bytes(data: &[u8], filename: &str, mime_type: &str) -> anyhow::Result<()> {
    let array = js_sys::Uint8Array::from(data);
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&array);

    let blob_props = BlobPropertyBag::new();
    blob_props.set_type(mime_type);

    let blob = Blob::new_with_u8_array_sequence_and_options(&blob_parts, &blob_props)
        .map_err(|e| anyhow::anyhow!("Failed to create blob for download: {:?}", e))?;
    let url = Url::create_object_url_with_blob(&blob)
        .map_err(|e| anyhow::anyhow!("Failed to create object URL for download blob: {:?}", e))?;

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let anchor = document
        .create_element("a")
        .map_err(|e| anyhow::anyhow!("Failed to create anchor element: {:?}", e))?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to cast anchor element to HtmlAnchorElement: {:?}",
                e
            )
        })?;

    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();

    Url::revoke_object_url(&url)
        .map_err(|e| anyhow::anyhow!("Failed to revoke object URL: {:?}", e))?;
    Ok(())
}
