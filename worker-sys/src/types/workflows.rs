use js_sys::{Array, Function, Promise};
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
    pub type WorkflowStep;

    #[wasm_bindgen(method, catch, js_name = do)]
    pub fn do_(this: &WorkflowStep, name: &str, callback: &Function) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = do)]
    pub fn do_with_config(
        this: &WorkflowStep,
        name: &str,
        config: &JsValue,
        callback: &Function,
    ) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    pub fn sleep(this: &WorkflowStep, name: &str, duration: &JsValue) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = sleepUntil)]
    pub fn sleep_until(
        this: &WorkflowStep,
        name: &str,
        timestamp: &JsValue,
    ) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = waitForEvent)]
    pub fn wait_for_event(
        this: &WorkflowStep,
        name: &str,
        options: &JsValue,
    ) -> Result<Promise, JsValue>;
}

#[wasm_bindgen(module = "cloudflare:workflows")]
extern "C" {
    #[wasm_bindgen(extends = js_sys::Error)]
    #[derive(Debug, Clone)]
    pub type NonRetryableError;

    #[wasm_bindgen(constructor)]
    pub fn new(message: &str) -> NonRetryableError;
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
