use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct FeatureFlags {
    flags: Arc<RwLock<HashMap<String, bool>>>,
}

impl FeatureFlags {
    pub fn new() -> Self {
        let mut flags = HashMap::new();
        flags.insert("parallel_subagents".to_string(), true);
        flags.insert("planning_tool".to_string(), true);
        flags.insert("filesystem_tools".to_string(), true);
        flags.insert("mcp_tools".to_string(), true);
        flags.insert("hitl_approval".to_string(), false);
        flags.insert("wasm_sandbox".to_string(), false);
        flags.insert("cost_dashboard".to_string(), true);
        Self { flags: Arc::new(RwLock::new(flags)) }
    }

    pub fn is_enabled(&self, flag: &str) -> bool {
        self.flags.read().unwrap().get(flag).copied().unwrap_or(false)
    }

    pub fn enable(&self, flag: &str) {
        self.flags.write().unwrap().insert(flag.to_string(), true);
    }

    pub fn disable(&self, flag: &str) {
        self.flags.write().unwrap().insert(flag.to_string(), false);
    }

    pub fn all_flags(&self) -> Vec<(String, bool)> {
        self.flags.read().unwrap().iter().map(|(k, v)| (k.clone(), *v)).collect()
    }
}
