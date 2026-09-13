use crate::models::{AppSettings, UnifiedNode};
use anyhow::Result;
use std::path::Path;

pub trait CoreAdapter: Send + Sync {
    fn generate_config(&self, node: &UnifiedNode, settings: &AppSettings, work_dir: &Path) -> Result<String> {
        self.generate_config_with_relay(node, None, settings, work_dir)
    }
    fn generate_config_with_relay(&self, node: &UnifiedNode, relay_node: Option<&UnifiedNode>, settings: &AppSettings, work_dir: &Path) -> Result<String>;
}
