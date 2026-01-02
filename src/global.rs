use once_cell::sync::Lazy;
use std::sync::{Mutex as StdMutex, MutexGuard as StdMutexGuard};
use tokio::sync::{Mutex, MutexGuard};

use crate::env::RuntimeModuleEnv;
use crate::event_bus::{RuntimeEvent, RuntimeEventBus, RuntimeEventListenerHandlerArg};
use crate::state::Locked;

static RUNTIME_MODULE_ENV: Lazy<StdMutex<Option<RuntimeModuleEnv<Locked>>>> =
    Lazy::new(|| StdMutex::new(None));
static RUNTIME_EVENT_BUS: Lazy<Mutex<Option<RuntimeEventBus>>> = Lazy::new(|| Mutex::new(None));
///
pub async fn init_runtime(env: RuntimeModuleEnv<Locked>) {
    let mut guard = RUNTIME_MODULE_ENV.lock().unwrap();
    *guard = Some(env);
    let mut event_buss_guard = RUNTIME_EVENT_BUS.lock().await;
    *event_buss_guard = Some(RuntimeEventBus::new());
}
/// Ayarlamalar sonrasında "RuntimeModuleEnv" nesnesine erişim için kullanılır.
pub fn runtime_env() -> StdMutexGuard<'static, Option<RuntimeModuleEnv<Locked>>> {
    RUNTIME_MODULE_ENV.lock().unwrap()
}
pub async fn with_event_bus_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut RuntimeEventBus) -> R,
{
    let mut guard = RUNTIME_EVENT_BUS.lock().await;
    let bus = guard.as_mut().expect("RuntimeEventBus Not initialized!");
    f(bus)
}
pub async fn emit_event<T: Send + Sync + 'static>(
    event: RuntimeEvent,
    arg: T,
) {
    let mut guard = RUNTIME_EVENT_BUS.lock().await;

    if let Some(bus) = guard.as_mut() {
        bus.emit(&event, arg).await;
    }
}
