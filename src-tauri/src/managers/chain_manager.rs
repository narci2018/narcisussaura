use crate::models::ProxyChain;
use parking_lot::RwLock;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ChainManager {
    chains: Arc<RwLock<Vec<ProxyChain>>>,
    data_path: PathBuf,
}

impl ChainManager {
    pub fn new(app_data_dir: &Path) -> Self {
        let data_path = app_data_dir.join("chains.json");
        let mut initial_chains = Vec::new();

        if data_path.exists() {
            if let Ok(content) = fs::read_to_string(&data_path) {
                if let Ok(loaded) = serde_json::from_str::<Vec<ProxyChain>>(&content) {
                    initial_chains = loaded;
                }
            }
        }

        if initial_chains.is_empty() {
            initial_chains.push(ProxyChain {
                id: "chain-sample-1".to_string(),
                name: "示例双跳链 (Trojan 中继 ➔ Reality 落地)".to_string(),
                remarks: "双跳前置中继链示例，首节点绕过限制，落地节点访问目标网络".to_string(),
                node_ids: vec![
                    "megav-nl-trojan-1".to_string(),
                    "megav-nl-reality-1".to_string(),
                ],
                latency_ms: None,
                created_at: chrono::Utc::now().timestamp_millis(),
            });
        }

        let mgr = Self {
            chains: Arc::new(RwLock::new(initial_chains)),
            data_path,
        };
        let _ = mgr.save();
        mgr
    }

    pub fn get_all(&self) -> Vec<ProxyChain> {
        self.chains.read().clone()
    }

    pub fn get_by_id(&self, id: &str) -> Option<ProxyChain> {
        self.chains.read().iter().find(|c| c.id == id).cloned()
    }

    pub fn add_chain(&self, mut chain: ProxyChain) -> Result<ProxyChain, String> {
        if chain.id.trim().is_empty() {
            chain.id = format!("chain-{}", Uuid::new_v4().to_string().chars().take(8).collect::<String>());
        }
        if chain.node_ids.len() < 2 {
            return Err("A proxy chain requires at least 2 nodes (entry hop and exit hop).".to_string());
        }
        if chain.created_at <= 0 {
            chain.created_at = chrono::Utc::now().timestamp_millis();
        }

        let mut lock = self.chains.write();
        lock.push(chain.clone());
        drop(lock);

        self.save()?;
        Ok(chain)
    }

    pub fn update_chain(&self, chain: ProxyChain) -> Result<(), String> {
        if chain.node_ids.len() < 2 {
            return Err("A proxy chain requires at least 2 nodes (entry hop and exit hop).".to_string());
        }

        let mut lock = self.chains.write();
        if let Some(pos) = lock.iter().position(|c| c.id == chain.id) {
            lock[pos] = chain;
            drop(lock);
            self.save()?;
            Ok(())
        } else {
            Err("Proxy chain not found".to_string())
        }
    }

    pub fn delete_chain(&self, id: &str) -> Result<(), String> {
        let mut lock = self.chains.write();
        let initial_len = lock.len();
        lock.retain(|c| c.id != id);
        if lock.len() < initial_len {
            drop(lock);
            self.save()?;
            Ok(())
        } else {
            Err("Proxy chain not found".to_string())
        }
    }

    pub fn update_latency(&self, id: &str, latency_ms: Option<i64>) -> Result<(), String> {
        let mut lock = self.chains.write();
        if let Some(chain) = lock.iter_mut().find(|c| c.id == id) {
            chain.latency_ms = latency_ms;
            drop(lock);
            self.save()?;
            Ok(())
        } else {
            Err("Proxy chain not found".to_string())
        }
    }

    fn save(&self) -> Result<(), String> {
        let lock = self.chains.read();
        let json_str = serde_json::to_string_pretty(&*lock)
            .map_err(|e| format!("Failed to serialize proxy chains: {}", e))?;
        if let Some(parent) = self.data_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&self.data_path, json_str)
            .map_err(|e| format!("Failed to save proxy chains: {}", e))?;
        Ok(())
    }
}
