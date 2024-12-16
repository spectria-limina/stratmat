use std::sync::{LazyLock, RwLock};

use bevy::{
    log::{debug_once, error, info},
    prelude::{Resource, *},
    utils::{HashMap, HashSet},
};
use custom_elements::CustomElement;
use itertools::Itertools;
use js_sys::WebAssembly::Global;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{console, HtmlElement, SvgElement, SvgImageElement, SvgsvgElement};

use super::Arena;
use crate::{asset::RootAssetPath, component::WebComponent, image::DrawImage};

#[derive(Default, Deref, DerefMut)]
pub struct ArenaWebComponents(HashMap<Entity, WebComponent>);

pub const ARENA_COMPONENT_TAG: &str = "stratmat-arena";
pub const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

static ARENA_COMPONENTS: LazyLock<RwLock<HashSet<String>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

impl ArenaWebComponents {
    pub fn sync_web_components(
        arena_q: Query<Entity, With<Arena>>,
        mut components_map: NonSendMut<ArenaWebComponents>,
    ) {
        let Ok(id) = arena_q.get_single() else {
            debug_once!("arena not yet initialized; not associating web component");
            return;
        };

        if components_map.contains_key(&id) {
            debug_once!("arena already associated; not associating web component");
        }

        let components = ARENA_COMPONENTS.read().unwrap();
        match components.len() {
            0 => {
                debug_once!("no web component yet; not associating to arena");
            }
            2.. => {
                error!(
                    "multiple <{ARENA_COMPONENT_TAG}> elements detected. I don't know how to \
                     handle that"
                );
            }
            1 => {
                components_map.insert(
                    id,
                    WebComponent::new(components.iter().exactly_one().unwrap()).unwrap(),
                );
                debug_once!("associating arena web component to element '#{id}'");
            }
        }
    }
}

impl CustomElement for ArenaWebComponents {
    fn connected_callback(&mut self, this: &HtmlElement) {
        let id = this.id();
        if id.is_empty() {
            error!("A <{ARENA_COMPONENT_TAG}> element must have an id");
            return;
        }
        info!("New <{ARENA_COMPONENT_TAG}> added with ID '{id}'");
        ARENA_COMPONENTS.write().unwrap().insert(id);
    }

    fn disconnected_callback(&mut self, _this: &HtmlElement) {
        console::info_1(&"disconnected an ArenaComponent".into())
    }

    fn inject_children(&mut self, _this: &HtmlElement) {
        console::info_1(&"injecting children for an ArenaComponent".into())
    }
}

impl Arena {
    pub fn display_web(
        q: Option<
            Single<
                (Entity, &Arena, &DrawImage, &GlobalTransform),
                Or<(Changed<Arena>, Changed<DrawImage>, Changed<GlobalTransform>)>,
            >,
        >,
        components: NonSend<ArenaWebComponents>,
        root_path: Res<RootAssetPath>,
    ) {
        let run = move || -> Result<(), JsValue> {
            let Some(q) = q else {
                debug_once!("can't display arena: no arena");
                return Ok(());
            };
            let (id, arena, draw, transform) = *q;

            let Some(web) = components.get(&id) else {
                debug_once!("can't display arena: no web component");
                return Ok(());
            };
            debug_once!("displaying arena");

            let document = web_sys::window()
                .ok_or_else(|| JsValue::from("no window!?"))?
                .document()
                .ok_or_else(|| JsValue::from("no document!?"))?;
            let svg = document
                .create_element_ns(Some(SVG_NAMESPACE), "svg")?
                .dyn_into::<SvgsvgElement>()?;
            svg.set_attribute(
                "viewBox",
                &format!(
                    "{} {} {} {}",
                    -arena.size.x / 2.0,
                    -arena.size.y / 2.0,
                    arena.size.x,
                    arena.size.y,
                ),
            );

            let image = document
                .create_element_ns(Some(SVG_NAMESPACE), "image")?
                .dyn_into::<SvgImageElement>()?;
            image
                .href()
                .set_base_val(root_path.join(&draw.path).to_str().unwrap());
            image.x().base_val().set_value(-draw.size.x / 2.0)?;
            image.y().base_val().set_value(-draw.size.y / 2.0)?;
            image.height().base_val().set_value(draw.size.x)?;
            image.width().base_val().set_value(draw.size.y)?;
            svg.append_child(&image)?;

            web.shadow_root.replace_children_with_node_1(&svg);
            Ok(())
        };
        if let Err(e) = run() {
            error!("{e:?}");
        }
    }
}
