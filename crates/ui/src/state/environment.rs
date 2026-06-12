//! Environment state: manages a workspace's environments and their variables.
//!
//! Split out of `ServiceState` so environment concerns live in their own entity.
//! Views (e.g. `EnvironmentSelector`) read this state and re-render automatically
//! via GPUI's notify tracking — no custom event type is needed.

use gpui::*;
use models::{Environment, EnvironmentVariable};
use register::GlobalStorage;
use uuid::Uuid;

pub struct EnvironmentState {
    pub workspace_id: Uuid,
    pub environments: Vec<Environment>,
    pub active_environment_id: Option<Uuid>,
}

impl EnvironmentState {
    pub fn new(workspace_id: Uuid, cx: &mut Context<Self>) -> Self {
        let storage = GlobalStorage::global(cx);

        let mut environments = storage
            .env
            .list_by_workspace(workspace_id)
            .unwrap_or_default();
        for env in &mut environments {
            env.variables = storage.var.list_by_environment(env.id).unwrap_or_default();
        }

        let active_environment_id = environments
            .iter()
            .find(|e| e.is_default)
            .or(environments.first())
            .map(|e| e.id);

        Self {
            workspace_id,
            environments,
            active_environment_id,
        }
    }

    // === Environment Operations ===

    pub fn create_environment(&mut self, name: String, cx: &mut Context<Self>) {
        let env = Environment::new(self.workspace_id, &name);

        let storage = GlobalStorage::global(cx);
        storage.env.insert(&env).ok();

        let is_first = self.environments.is_empty();
        self.environments.push(env.clone());
        if is_first {
            self.active_environment_id = Some(env.id);
        }
        cx.notify();
    }

    pub fn update_environment(&mut self, env: Environment, cx: &mut Context<Self>) {
        let storage = GlobalStorage::global(cx);
        storage.env.update(&env).ok();

        if let Some(existing) = self.environments.iter_mut().find(|e| e.id == env.id) {
            existing.name = env.name;
            existing.base_url = env.base_url;
            existing.is_default = env.is_default;
        }
        cx.notify();
    }

    pub fn delete_environment(&mut self, env_id: Uuid, cx: &mut Context<Self>) {
        let storage = GlobalStorage::global(cx);
        storage.env.delete(env_id).ok();

        self.environments.retain(|e| e.id != env_id);
        if self.active_environment_id == Some(env_id) {
            self.active_environment_id = self.environments.first().map(|e| e.id);
        }
        cx.notify();
    }

    pub fn set_active_environment(&mut self, env_id: Uuid, cx: &mut Context<Self>) {
        self.active_environment_id = Some(env_id);
        cx.notify();
    }

    pub fn get_active_environment(&self) -> Option<&Environment> {
        self.active_environment_id
            .and_then(|id| self.environments.iter().find(|e| e.id == id))
    }

    // === Environment Variable Operations ===

    pub fn add_environment_variable(
        &mut self,
        env_id: Uuid,
        var: EnvironmentVariable,
        cx: &mut Context<Self>,
    ) {
        let storage = GlobalStorage::global(cx);
        storage.var.insert(&var).ok();

        if let Some(env) = self.environments.iter_mut().find(|e| e.id == env_id) {
            env.variables.push(var);
        }
        cx.notify();
    }

    pub fn update_environment_variable(
        &mut self,
        var: EnvironmentVariable,
        cx: &mut Context<Self>,
    ) {
        let storage = GlobalStorage::global(cx);
        storage.var.update(&var).ok();

        for env in &mut self.environments {
            if let Some(existing) = env.variables.iter_mut().find(|v| v.id == var.id) {
                *existing = var.clone();
                break;
            }
        }
        cx.notify();
    }

    pub fn delete_environment_variable(&mut self, var_id: Uuid, cx: &mut Context<Self>) {
        let storage = GlobalStorage::global(cx);
        storage.var.delete(var_id).ok();

        for env in &mut self.environments {
            env.variables.retain(|v| v.id != var_id);
        }
        cx.notify();
    }
}
