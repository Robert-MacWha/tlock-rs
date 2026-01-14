use wasm_bindgen::JsCast;

pub fn blur_active_element() {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(active) = document.active_element() {
                if let Ok(html_element) = active.dyn_into::<web_sys::HtmlElement>() {
                    let _ = html_element.blur();
                }
            }
        }
    }
}
