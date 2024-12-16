use bevy::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, HtmlElement, ShadowRoot};

pub struct WebComponent {
    pub element: HtmlElement,
    pub shadow_root: ShadowRoot,
}

impl WebComponent {
    pub fn new(id: &str) -> Result<WebComponent, JsValue> {
        let element = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .query_selector(&format!("#{id}"))?;
        let Some(element) = element else {
            return Err(format!("no element matched selector '#{id}'").into());
        };

        let Some(shadow_root) = element.shadow_root() else {
            return Err(format!("element '#{id}' does not have a shadow root").into());
        };

        Ok(WebComponent {
            element: element.dyn_into::<HtmlElement>()?,
            shadow_root,
        })
    }
}
