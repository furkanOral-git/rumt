use rumt::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;

mod common;
use common::{InventoryService, TestPayload};

use crate::common::setup_runtime;

#[tokio::test]
async fn test_event_bus_flow() {

    setup_runtime().await;
    
    // I. Hazırlık: Veriyi depolamak için bir liste oluştur
    let storage = Arc::new(Mutex::new(Vec::new()));
    
    // II. Servisi Başlat
    let service = InventoryService::new(Arc::clone(&storage));
    
    // Makro tarafından oluşturulan .init() metodunu çağırıyoruz
    // Bu metod servisi Event Bus'a kaydeder.
    let _controller = service.init().await;

    // III. Olayı Tetikle
    let event = rumt::event_bus::RuntimeEvent::Static { 
        event_name: "order.created".into() 
    };
    let payload = TestPayload { 
        data: "Merhaba Rust!".into() 
    };

    println!("Event yayılıyor...");
    rumt::global::emit_event(event, &payload).await;

    // IV. Doğrulama
    // Event bus asenkron çalıştığı için verinin gelmiş olması gerekir
    let final_data = storage.lock().await;
    assert_eq!(final_data.len(), 1);
    assert_eq!(final_data[0], "Merhaba Rust!");
    
    println!("Test başarıyla tamamlandı!");
}