use std::path::{Path, PathBuf};
use std::{collections::HashMap, marker::PhantomData};

use crate::app_info::AppInfo;
use crate::state::{Locked, Unlocked};

pub struct RuntimeModuleEnv<State> {
    pub state: PhantomData<State>,
    pub paths: HashMap<String, String>,
    pub app: Option<AppInfo>,
}

impl RuntimeModuleEnv<Unlocked> {
    
    /// # Example
    ///
    /// Aşağıdaki örnek, `RuntimeModuleEnv` yapısını kullanarak çalışma zamanını
    /// nasıl başlatacağınızı göstermektedir.
    ///
    /// ```
    ///
    /// // 1. Builder'ı Unlocked state ile başlatın
    /// async fn setup_runtime() {
    /// 
    /// let mut env_builder = rumt::env::RuntimeModuleEnv::<Unlocked>::new();
    /// 
    /// env_builder.add_app_info("MyApp", "MyCompany", "com");
    /// env_builder.insert_path("db", "/tmp/test.db");
    /// 
    /// let locked_env = env_builder.lock_env();
    /// init_runtime(locked_env).await;
    /// 
    /// // 4. Daha sonra global runtime'a erişin
    /// let runtime_env_guard = runtime_env();
    /// let runtime_env = runtime_env_guard.as_ref().unwrap();
    /// assert_eq!(runtime_env.app.as_ref().unwrap().app_name, "MyApp");
    /// ```
    /// 
    pub fn new() -> Self {
        Self {
            state: PhantomData,
            paths: HashMap::new(),
            app: None,
        }
    }

    pub fn insert_path(mut self, name: impl Into<String>, path: impl Into<String>) -> Self {
        self.paths.insert(name.into(), path.into());
        self
    }

    pub fn add_app_info(
        mut self,
        name: impl Into<String>,
        company: impl Into<String>,
        qualifier: impl Into<String>,
    ) -> Self {
        self.app = Some(AppInfo {
            app_name: name.into(),
            company: company.into(),
            qualifier: qualifier.into(),
        });
        self
    }

    pub fn lock_env(mut self) -> RuntimeModuleEnv<Locked> {
        let app = self.app.expect("AppInfo must be set before locking!");
        RuntimeModuleEnv {
            state: PhantomData,
            paths: self.paths,
            app: Some(app),
        }
    }
}