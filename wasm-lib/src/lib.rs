use space::Space;
use wasm_bindgen::{prelude::*, JsCast};

mod space;

static mut SPACE: *mut Space = std::ptr::null_mut::<Space>();

#[wasm_bindgen]
pub fn init_space() {
    wasm_logger::init(wasm_logger::Config::default());

    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document
        .get_element_by_id("canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| ())
        .unwrap();
    let ctx = canvas
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .unwrap();

    unsafe {
        SPACE = Box::leak(Box::new(Space::new(ctx, 200, 100, 1. / 60.)));
    }
}

#[wasm_bindgen]
pub fn step() {
    unsafe {
        (*SPACE).step();
    }
}

#[wasm_bindgen]
pub fn render() {
    unsafe {
        (*SPACE).render();
    }
}
