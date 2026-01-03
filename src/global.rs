use once_cell::sync::Lazy;
use std::sync::{Mutex as StdMutex, MutexGuard as StdMutexGuard};
use tokio::sync::{Mutex};

use crate::{Locked, RuntimeModuleEnv, event_bus::{RuntimeEventBus,RuntimeEvent}}; // Sadece Mutex yeterli

// ... diğer importlar

static RUNTIME_MODULE_ENV: Lazy<StdMutex<Option<RuntimeModuleEnv<Locked>>>> =
    Lazy::new(|| StdMutex::new(None));

// Option kullanman doğru, çünkü bus sonradan init ediliyor.
pub(crate) static RUNTIME_EVENT_BUS: Lazy<Mutex<Option<RuntimeEventBus>>> = Lazy::new(|| Mutex::new(None));

pub async fn init_runtime(env: RuntimeModuleEnv<Locked>) {
    let mut guard = RUNTIME_MODULE_ENV.lock().unwrap();
    *guard = Some(env);
    
    let mut event_bus_guard = RUNTIME_EVENT_BUS.lock().await;
    // Eğer zaten init edilmişse tekrar etmemek için kontrol
    if event_bus_guard.is_none() {
        *event_bus_guard = Some(RuntimeEventBus::new());
    }
}
pub fn runtime_env() -> StdMutexGuard<'static, Option<RuntimeModuleEnv<Locked>>> {
    RUNTIME_MODULE_ENV.lock().unwrap()
}

pub async fn emit_event<T: Send + Sync + 'static>(event: RuntimeEvent, arg: T) {
    let mut guard = RUNTIME_EVENT_BUS.lock().await;
    if let Some(bus) = guard.as_mut() {
        // Burada bus.emit asenkron olduğu için guard'ı tutarken await ediyoruz.
        // tokio Mutex kullandığın için bu güvenlidir.
        bus.emit(&event, arg).await;
    }
}