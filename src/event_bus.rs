use futures::future::BoxFuture;
use std::{any::Any, collections::HashMap, sync::Arc};

// --- Temel Tipler ve Traitler ---

#[derive(PartialEq, Eq, Debug, Hash, Clone)]
pub enum RuntimeEvent {
    OnceTriggered { event_name: String },
    Static { event_name: String },
}

pub trait RuntimeEventListenerHandlerArg: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + Send + Sync> RuntimeEventListenerHandlerArg for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl dyn RuntimeEventListenerHandlerArg {
    pub fn downcast<T: Any>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}

pub(crate) type RuntimeEventListenerHandler =
    Box<dyn Fn(&dyn RuntimeEventListenerHandlerArg) -> BoxFuture<'static, ()> + Send + Sync>;

pub struct RuntimeEventListener {
    pub(crate) tag: String,
    pub(crate) handler: RuntimeEventListenerHandler,
}

impl RuntimeEventListener {
    pub fn new(tag: impl Into<String>, handler: RuntimeEventListenerHandler) -> Self {
        Self {
            tag: tag.into(),
            handler,
        }
    }
}

// --- Event Bus Merkezi ---
#[doc(hidden)] // Kullanıcı dökümanında ve kod tamamlamada gözükmez
pub struct RuntimeEventBus {
    pub(crate) pairs: HashMap<RuntimeEvent, Vec<RuntimeEventListener>>,
}

impl RuntimeEventBus {
    pub(crate) fn new() -> Self {
        Self {
            pairs: HashMap::new(),
        }
    }

    /// Makronun kütüphane dışından erişebilmesi için teknik olarak pub olmalı.
    /// Ancak dökümantasyonda gizleyerek kullanıcıdan saklıyoruz.
    #[doc(hidden)]
    pub async fn with_instance_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        // Global'deki asenkron Mutex'i kilitliyoruz
        let mut guard = crate::global::RUNTIME_EVENT_BUS.lock().await;
        let bus = guard.as_mut().expect("RuntimeEventBus Not initialized! Call init_runtime first.");
        f(bus)
    }

    pub fn add_listener(&mut self, event: RuntimeEvent, listener: RuntimeEventListener) {
        self.pairs.entry(event).or_insert(vec![]).push(listener);
    }

    pub async fn emit<T: Send + Sync + 'static>(&mut self, event: &RuntimeEvent, arg: T) {
        // Sıfır kopya: Veri bir kez Arc içine alınır
        let shared_payload = std::sync::Arc::new(arg);

        if let Some(listeners) = self.pairs.get(event) {
            for listener in listeners {
                // Her handler'a verinin pointer'ı (Arc) gönderilir
                (listener.handler)(&shared_payload).await;
            }
        }

        // Tek seferlik eventlerin temizlenmesi
        if let RuntimeEvent::OnceTriggered { .. } = event {
            self.pairs.remove(event);
        }
    }

    pub fn remove_all_listeners_by_tag(&mut self, tag: &str) {
        for listeners in self.pairs.values_mut() {
            listeners.retain(|l| l.tag != tag);
        }
    }
}

// --- Trait Tanımları ---

pub trait RuntimeEventListenerTrait: Send + Sync {
    fn dispose_self(&self) -> BoxFuture<'static, ()>;
}

pub trait RuntimeEventListenerInitializer: Sized {
    fn init(self) -> BoxFuture<'static, Arc<dyn RuntimeEventListenerTrait>>;
}


/// # Event Bus Kullanım Senaryosu: Sipariş ve Bildirim Sistemi
///
/// Bu senaryoda, bir sipariş tamamlandığında hem loglama yapan hem de dış dünyaya
/// asenkron olarak bildirim gönderen bir sistem simüle edilmiştir.
///
/// ## 1. Veri Yapılarının Tanımlanması
///
/// Olay argümanı olarak kullanılacak struct'lar `RuntimeEventListenerHandlerArg`
/// trait'ini implemente etmelidir (Makromuz bunu otomatik yapar).
///
/// ```rust
/// #[derive(Clone)]
/// pub struct OrderEvent {
///     pub order_id: u64,
///     pub total_amount: f64,
///     pub customer_email: String,
/// }
/// ```
///
/// ## 2. Dinleyici (Listener) Yapısı
///
/// İş mantığını yürütecek olan servis. İçerisinde hem senkron hem asenkron metodlar barındırabilir.
///
/// ```rust
/// pub struct NotificationService {
///     sender_name: String,
/// }
///
/// impl NotificationService {
///     pub fn new(name: &str) -> Self {
///         Self { sender_name: name.into() }
///     }
///
///     /// Senkron Handler: Hızlıca log basar.
///     fn log_order(&self, event: &OrderEvent) {
///         println!("[{}] Sipariş alındı: ID #{}", self.sender_name, event.order_id);
///     }
///
///     /// Asenkron Handler: Dış servise HTTP isteği atar veya dosyaya yazar.
///     async fn send_email(&self, event: &OrderEvent) {
///         // Gerçek bir senaryoda: tokio::time::sleep veya reqwest::post().await
///         println!("E-posta gönderiliyor: {} adresine {} TL tutarında fatura iletildi.",
///             event.customer_email, event.total_amount);
///     }
/// }
/// ```
///
/// ## 3. Makro ile Bağlantı
///
/// `event_handlers!` makrosu ile metodlar olaylara bağlanır.
///
/// ```rust
/// event_handlers! {
///     NotificationService;
///     RuntimeEvent::Static { event_name: "order.completed".into() } => log_order : OrderEvent,
///     RuntimeEvent::Static { event_name: "order.completed".into() } => async send_email : OrderEvent
/// }
/// ```
///
/// ## 4. Uygulama Akışı
///
/// Sistemin asenkron olarak başlatılması ve olayın tetiklenmesi.
///
/// ```rust
/// #[tokio::main]
/// async fn main() {
///     // I. Runtime ve Global State Başlatma
///     let env = RuntimeModuleEnv::new(); // Örnek env
///     global::init_runtime(env).await;
///
///     // II. Servisin Kaydedilmesi
///     let service = NotificationService::new("Otomatik Servis");
///     let _controller = NotificationService::init(service).await;
///
///     // III. Olayın Tetiklenmesi
///     let order_info = OrderEvent {
///         order_id: 12345,
///         total_amount: 1550.50,
///         customer_email: "test@example.com".into(),
///     };
///
///     println!("Olay tetikleniyor...");
///     
///     // Tüm handler'lar (log_order ve send_email) sırayla çalışır.
///     global::emit_event(
///         RuntimeEvent::Static { event_name: "order.completed".into() },
///         &order_info
///     ).await;
///
///     println!("Tüm süreç tamamlandı.");
/// }
/// ```
///
/// # Sıralılık Garantisi
/// Yukarıdaki örnekte `log_order` bittikten sonra `send_email` başlar. `emit_event().await`
/// satırı ancak her iki işlem de tamamen bittiğinde bir alt satıra geçer.
#[macro_export]
macro_rules! event_handlers {
    // Giriş kolları: Async veya normal yazımı destekler
    ($struct_name:ty; $($event:expr => $handler:ident : $arg:ty),* $(,)?) => {
        $crate::event_handlers!(@impl $struct_name; $($event => $handler : $arg),*);
    };
    ($struct_name:ty; $($event:expr => async $handler:ident : $arg:ty),* $(,)?) => {
        $crate::event_handlers!(@impl $struct_name; $($event => $handler : $arg),*);
    };

    // Merkezi Uygulama Mantığı
    (@impl $struct_name:ty; $( $event_variant:expr => $handler_fn:ident : $arg_type:ty ),*) => {
        impl $crate::event_bus::RuntimeEventListenerTrait for $struct_name {
            fn dispose_self(&self) -> $crate::futures::future::BoxFuture<'static, ()> {
                let tag = std::stringify!($struct_name);
                std::boxed::Box::pin(async move {
                    // Dispose sırasında global bus'a güvenli asenkron erişim
                    $crate::event_bus::RuntimeEventBus::with_instance_mut(|bus| {
                        bus.remove_all_listeners_by_tag(tag);
                    }).await;
                })
            }
        }

        impl $crate::event_bus::RuntimeEventListenerInitializer for $struct_name {
            fn init(self) -> $crate::futures::future::BoxFuture<'static, std::sync::Arc<dyn $crate::event_bus::RuntimeEventListenerTrait>> {
                let service = std::sync::Arc::new(self);
                let service_clone = std::sync::Arc::clone(&service);
                let struct_tag = std::stringify!($struct_name);

                std::boxed::Box::pin(async move {
                    // Kayıt sırasında global bus'a asenkron erişim
                    $crate::event_bus::RuntimeEventBus::with_instance_mut(|bus| {
                        $(
                            let arc_clone = std::sync::Arc::clone(&service_clone);
                            let event = $event_variant;

                            let handler = std::boxed::Box::new(move |args: &dyn $crate::event_bus::RuntimeEventListenerHandlerArg| {
                                let arc_inner = std::sync::Arc::clone(&arc_clone);
                                // Veri downcast edilirken Arc<$arg_type> olarak karşılanır
                                let maybe_shared = args.downcast::<std::sync::Arc<$arg_type>>().map(|a| std::sync::Arc::clone(a));
                                
                                std::boxed::Box::pin(async move {
                                    if let Some(shared_data) = maybe_shared {
                                        // Downcast başarılıysa servis metodunu çağır
                                        arc_inner.$handler_fn(&shared_data).await;
                                    }
                                }) as $crate::futures::future::BoxFuture<'static, ()>
                            });

                            let listener = $crate::event_bus::RuntimeEventListener::new(struct_tag, handler);
                            bus.add_listener(event, listener);
                        )*
                    }).await;

                    service as std::sync::Arc<dyn $crate::event_bus::RuntimeEventListenerTrait>
                })
            }
        }
    };
}