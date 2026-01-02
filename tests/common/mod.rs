use rumt::{Unlocked, init_runtime, prelude::*};
use std::sync::Arc;
use tokio::sync::Mutex;

// 1. Veri Yapısı (Mutlaka Clone olmalı)
#[derive(Clone, Debug)]
pub struct TestPayload {
    pub data: String,
}
/// Runtime'ı başlatan yardımcı fonksiyon
pub async fn setup_runtime() {
    // Rust'ta method chaining (&mut Self) sonrası sahipliği almak için 
    // değişkeni önce oluşturup sonra metodları çağırmak daha güvenlidir.
    let env = rumt::env::RuntimeModuleEnv::<Unlocked>::new()
    .add_app_info("MyApp", "MyCompany", "com")
    .insert_path("db", "/tmp/test.db")
    .lock_env();
    
    init_runtime(env).await;
}
// 2. Servis Yapısı
pub struct InventoryService {
    // Test sırasında veriyi doğrulamak için Mutex kullanıyoruz
    pub received_data: Arc<Mutex<Vec<String>>>,
}

impl InventoryService {
    pub fn new(storage: Arc<Mutex<Vec<String>>>) -> Self {
        Self { received_data: storage }
    }

    // Handler Metodu: &TestPayload (referans) bekler
    pub async fn handle_order(&self, arg: &TestPayload) {
        let mut data = self.received_data.lock().await;
        data.push(arg.data.clone());
        println!("Event alındı: {}", arg.data);
    }
}

// 3. Makro Çağrısı (Tipin tanımlandığı yerde olmalı)
rumt::event_handlers! {
    InventoryService;
    rumt::event_bus::RuntimeEvent::Static { event_name: "order.created".into() } => async handle_order : TestPayload
}