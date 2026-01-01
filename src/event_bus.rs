use crate::global::with_event_bus_mut;
use futures::future::BoxFuture;
use std::{any::Any, collections::HashMap, sync::Arc};

// --- Temel Tipler ve Traitler ---

/// Çalışma zamanı olaylarının türlerini belirler.
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
pub enum RuntimeEvent {
    /// Sadece bir kez tetiklenen olaylar. Tetiklendikten sonra Bus'tan otomatik olarak silinir.
    OnceTriggered { event_name: String },
    /// Uygulama boyunca defalarca tetiklenebilen kalıcı olaylar.
    Static { event_name: String },
}

/// Olaylarla birlikte gönderilecek argümanlar için temel trait.
pub trait RuntimeEventListenerHandlerArg: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
}
// Any, Send,Sync trait'lerini implemente eden her tip bir RuntimeEventListenerHandlerArg olabilir.
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

/// Her türlü handler (sync/async) artık bir Future döndüren bir yapıdadır.
pub(crate) type RuntimeEventListenerHandler =
    Box<dyn Fn(&dyn RuntimeEventListenerHandlerArg) -> BoxFuture<'static, ()> + Send + Sync>;

/// Bir olayı ve o olay gerçekleştiğinde çalışacak mantığı temsil eden yapı.
pub(crate) struct RuntimeEventListener {
    pub(crate) tag: String,
    pub(crate) handler: RuntimeEventListenerHandler,
}

impl RuntimeEventListener {
    pub(crate) fn new(tag: impl Into<String>, handler: RuntimeEventListenerHandler) -> Self {
        Self {
            tag: tag.into(),
            handler,
        }
    }
}

// --- Event Bus Merkezi ---

pub(crate) struct RuntimeEventBus {
    pairs: HashMap<RuntimeEvent, Vec<RuntimeEventListener>>,
}

impl RuntimeEventBus {
    pub(crate) fn new() -> Self {
        Self {
            pairs: HashMap::new(),
        }
    }

    pub(crate) fn add_listener(&mut self, event: RuntimeEvent, listener: RuntimeEventListener) {
        self.pairs.entry(event).or_insert(vec![]).push(listener);
    }

    /// Olayları sırayla tetikler ve asenkron olarak her birinin bitmesini bekler.
    pub(crate) async fn emit(
        &mut self,
        event: &RuntimeEvent,
        arg: &dyn RuntimeEventListenerHandlerArg,
    ) {
        if let Some(listeners) = self.pairs.get(event) {
            for listener in listeners {
                // Burada .await diyerek handler'ın (sync veya async) bitmesini bekliyoruz.
                (listener.handler)(arg).await;
            }
        }

        if let RuntimeEvent::OnceTriggered { .. } = event {
            self.pairs.remove(event);
        }
    }

    pub(crate) fn remove_all_listeners_by_tag(&mut self, tag: &str) {
        for listeners in self.pairs.values_mut() {
            listeners.retain(|l| l.tag != tag);
        }
    }
}

// --- Trait Tanımı ---

pub trait RuntimeEventListenerTrait: Send + Sync {
    fn dispose_self(&self) -> impl std::future::Future<Output = ()> + Send;
}

// --- Makrolar ---

/// Yardımcı iç makro: async yazıldıysa .await ekler, yazılmadıysa düz çağırır.
#[macro_export]
macro_rules! handle_call {
    ($obj:ident, $func:ident, $arg:ident, async) => {
        $obj.$func($arg).await
    };
    ($obj:ident, $func:ident, $arg:ident, ) => {
        $obj.$func($arg)
    };
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
    (
        $struct_name:ty;
        $( $event_variant:pat => $( $is_async:ident )? $handler_fn:ident : $arg_type:ty ),* $(,)?
    ) => {
        impl $crate::event_bus::RuntimeEventListenerTrait for $struct_name {
            fn async dispose_self(&self) {
                $crate::global::with_event_bus_mut(|bus| {
                    bus.remove_all_listeners_by_tag(stringify!($struct_name));
                }).await;
            }
        }

        impl $struct_name {

            pub async fn init(instance: Self) -> std::sync::Arc<dyn $crate::event_bus::RuntimeEventListenerTrait> {
                
                let service = std::sync::Arc::new(instance);
                // register_internal artık asenkron ve await ediliyor
                // Kayıt işlemini arka planda yap, init hemen dönsün
                
                Self::register_internal(std::sync::Arc::clone(&service)).await;
                service as std::sync::Arc<dyn $crate::event_bus::RuntimeEventListenerTrait>
            }

            async fn register_internal(self_arc: std::sync::Arc<Self>) {
                $crate::global::with_event_bus_mut(|bus| {
                    let struct_tag = stringify!($struct_name);

                    $(
                        let arc_clone = std::sync::Arc::clone(&self_arc);
                        let event = $event_variant;

                        let handler = Box::new(move |args: &dyn $crate::event_bus::RuntimeEventListenerHandlerArg| {
                            let arc_inner = std::sync::Arc::clone(&arc_clone);
                            Box::pin(async move {
                                if let Some(typed_arg) = args.downcast::<$arg_type>() {
                                    $crate::handle_call!(arc_inner, $handler_fn, typed_arg, $($is_async)?);
                                }
                            }) as futures::future::BoxFuture<'static, ()>
                        });

                        let listener = $crate::event_bus::RuntimeEventListener::new(struct_tag, handler);
                        bus.add_listener(event, listener);
                    )*
                }).await;
            }
        }
    };
}
