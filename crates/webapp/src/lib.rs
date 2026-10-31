use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn boot() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn version() -> String {
    voxel::version().to_string()
}
