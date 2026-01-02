use crate::global::with_event_bus_mut;
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

pub struct RuntimeEventBus {
    pairs: HashMap<RuntimeEvent, Vec<RuntimeEventListener>>,
}

impl RuntimeEventBus {
    pub(crate) fn new() -> Self {
        Self {
            pairs: HashMap::new(),
        }
    }

    pub fn add_listener(&mut self, event: RuntimeEvent, listener: RuntimeEventListener) {
        self.pairs.entry(event).or_insert(vec![]).push(listener);
    }

    pub(crate) async fn emit<T: Send + Sync + 'static>(&mut self, event: &RuntimeEvent, arg: T) {
    // 1. Veriyi mülkiyetiyle alıp tek bir Arc içine koyuyoruz (Sıfır kopya başlangıcı)
    let shared_payload = std::sync::Arc::new(arg);

    if let Some(listeners) = self.pairs.get(event) {
        for listener in listeners {
            // 2. Arc<T>'yi &dyn RuntimeEventListenerHandlerArg olarak geçiyoruz.
            // Makro tarafı bunu downcast::<Arc<T>>() ile karşılayacak.
            (listener.handler)(&shared_payload).await;
        }
    }

    // 3. OnceTriggered kontrolü: Eğer event bir kez tetiklenecekse tüm listeyi temizle.
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

// --- Makro ---

#[macro_export]
macro_rules! event_handlers {
    // Giriş Kolları
    ($struct_name:ty; $($event:expr => $handler:ident : $arg:ty),* $(,)?) => {
        $crate::event_handlers!(@impl $struct_name; $($event => $handler : $arg),*);
    };
    ($struct_name:ty; $($event:expr => async $handler:ident : $arg:ty),* $(,)?) => {
        $crate::event_handlers!(@impl $struct_name; $($event => $handler : $arg),*);
    };

    // Merkezi Uygulama
    (@impl $struct_name:ty; $( $event_variant:expr => $handler_fn:ident : $arg_type:ty ),*) => {
        impl $crate::event_bus::RuntimeEventListenerTrait for $struct_name {
            fn dispose_self(&self) -> $crate::futures::future::BoxFuture<'static, ()> {
                let tag = std::stringify!($struct_name).to_string();
                std::boxed::Box::pin(async move {
                    $crate::global::with_event_bus_mut(|bus| {
                        bus.remove_all_listeners_by_tag(&tag);
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
                    $crate::global::with_event_bus_mut(|bus| {
                        $(
                            let arc_clone = std::sync::Arc::clone(&service_clone);
                            let event = $event_variant;

                            let handler = std::boxed::Box::new(move |args: &dyn $crate::event_bus::RuntimeEventListenerHandlerArg| {
                                
                                let arc_inner = std::sync::Arc::clone(&arc_clone);
                                
                                // Bus içinde Arc içine alınan veriyi burada downcast ediyoruz.
                                // Sadece pointer clone edilir, asıl veri kopyalanmaz.
                                let maybe_shared = args.downcast::<std::sync::Arc<$arg_type>>().map(|a| std::sync::Arc::clone(a));
                                
                                std::boxed::Box::pin(async move {
                                    if let Some(shared_data) = maybe_shared {
                                        // shared_data bir Arc<T>'dir. 
                                        // Deref sayesinde handler metodun &T alıyorsa sorunsuz çalışır.
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