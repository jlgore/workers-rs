use js_sys::{Array, Promise};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = js_sys::Object)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub type Workflow;

    #[wasm_bindgen(method, catch, js_name = get)]
    pub fn get(this: &Workflow, id: &str) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = create)]
    pub fn create(this: &Workflow, options: &JsValue) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = createBatch)]
    pub fn create_batch(this: &Workflow, batch: &Array) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = deleteBatch)]
    pub fn delete_batch(this: &Workflow, instance_ids: &Array) -> Result<Promise, JsValue>;
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = js_sys::Object)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub type WorkflowInstance;

    #[wasm_bindgen(method, getter)]
    pub fn id(this: &WorkflowInstance) -> String;

    #[wasm_bindgen(method, catch)]
    pub fn pause(this: &WorkflowInstance) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    pub fn resume(this: &WorkflowInstance) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    pub fn terminate(this: &WorkflowInstance, options: &JsValue) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    pub fn restart(this: &WorkflowInstance, options: &JsValue) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = delete)]
    pub fn delete(this: &WorkflowInstance) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    pub fn status(this: &WorkflowInstance) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = sendEvent)]
    pub fn send_event(this: &WorkflowInstance, event: &JsValue) -> Result<Promise, JsValue>;
}
