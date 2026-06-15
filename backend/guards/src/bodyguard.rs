use std::collections::HashMap;
use std::sync::Arc;

use agenticos_core::error::{AgentError, Result};
use chrono::Utc;
use dashmap::DashMap;
use serde_json::Value;

pub struct Bodyguard {
    rate_limiter: Arc<RateLimiter>,
    role_permissions: HashMap<String, Vec<String>>,
    user_roles: HashMap<String, String>,
}

struct RateLimiter {
    limits: DashMap<String, Vec<i64>>,
    max_requests: u32,
    window_seconds: i64,
}

impl RateLimiter {
    fn new(max_requests: u32, window_seconds: i64) -> Self {
        Self {
            limits: DashMap::new(),
            max_requests,
            window_seconds,
        }
    }

    fn check(&self, tool_name: &str, user_id: &str, workspace_id: &str) -> Result<()> {
        let key = format!("{}:{}:{}", tool_name, user_id, workspace_id);
        let now = Utc::now().timestamp();

        let mut entry = self.limits.entry(key).or_insert_with(Vec::new);
        let cutoff = now - self.window_seconds;

        entry.retain(|&ts| ts > cutoff);
        entry.push(now);

        if entry.len() as u32 > self.max_requests {
            return Err(AgentError::GuardViolation(format!(
                "Rate limit exceeded: {} requests per {} seconds for tool '{}'",
                self.max_requests, self.window_seconds, tool_name
            )));
        }

        Ok(())
    }
}

impl Bodyguard {
    pub fn new() -> Self {
        let mut role_permissions = HashMap::new();
        role_permissions.insert(
            "viewer".to_string(),
            vec!["viewer_tools".to_string()],
        );
        role_permissions.insert(
            "member".to_string(),
            vec!["viewer_tools".to_string(), "member_tools".to_string()],
        );
        role_permissions.insert(
            "admin".to_string(),
            vec![
                "viewer_tools".to_string(),
                "member_tools".to_string(),
                "admin_tools".to_string(),
            ],
        );
        role_permissions.insert(
            "owner".to_string(),
            vec![
                "viewer_tools".to_string(),
                "member_tools".to_string(),
                "admin_tools".to_string(),
            ],
        );

        Self {
            rate_limiter: Arc::new(RateLimiter::new(100, 60)),
            role_permissions,
            user_roles: HashMap::new(),
        }
    }

    pub fn with_rate_limits(mut self, max_requests: u32, window_seconds: i64) -> Self {
        self.rate_limiter = Arc::new(RateLimiter::new(max_requests, window_seconds));
        self
    }

    pub fn assign_role(&mut self, user_id: &str, role: &str) {
        self.user_roles.insert(user_id.to_string(), role.to_string());
    }

    pub fn set_role_permissions(&mut self, role: &str, scopes: Vec<String>) {
        self.role_permissions.insert(role.to_string(), scopes);
    }

    pub fn check_tool_access(
        &self,
        tool_name: &str,
        _args: &Value,
        user_id: &str,
        workspace_id: &str,
        tool_scope: &str,
    ) -> Result<()> {
        let role = self
            .user_roles
            .get(user_id)
            .ok_or_else(|| AgentError::Forbidden("User has no assigned role".to_string()))?;

        let allowed_scopes = self
            .role_permissions
            .get(role.as_str())
            .ok_or_else(|| {
                AgentError::Forbidden(format!("Role '{}' has no permissions defined", role))
            })?;

        if !allowed_scopes.iter().any(|s| s == tool_scope) {
            return Err(AgentError::Forbidden(format!(
                "Role '{}' does not have access to scope '{}'",
                role, tool_scope
            )));
        }

        self.rate_limiter.check(tool_name, user_id, workspace_id)?;

        Ok(())
    }
}

impl Default for Bodyguard {
    fn default() -> Self {
        Self::new()
    }
}
