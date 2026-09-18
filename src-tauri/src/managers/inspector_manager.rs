use anyhow::{bail, Result};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration, Instant};

use crate::models::{AppSettings, UnifiedNode};
use crate::core::SingBoxAdapter;
use std::os::windows::process::CommandExt;

#[derive(Clone, serde::Serialize)]
pub struct InspectProgress {
    pub current: usize,
    pub total: usize,
    pub status: String,
}

pub struct InspectorManager {
    app_data_dir: PathBuf,
}

impl InspectorManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self { app_data_dir }
    }

    pub async fn start_deep_inspection(
        &self,
        nodes: Vec<UnifiedNode>,
        settings: AppSettings,
        app: AppHandle,
    ) -> Result<Vec<UnifiedNode>> {
        let mut results = nodes.clone();
        if results.is_empty() {
            return Ok(results);
        }

        let emit_progress = |current: usize, total: usize, status: &str| {
            let _ = app.emit("inspector:progress", InspectProgress {
                current,
                total,
                status: status.to_string(),
            });
        };

        emit_progress(0, results.len(), "Preparing inspector...");

        let chunk_size = 20;
        let total_nodes = results.len();
        let mut tested_count = 0;

        for chunk in results.chunks_mut(chunk_size) {
            let chunk_len = chunk.len();
            emit_progress(tested_count, total_nodes, &format!("Generating config for {} nodes...", chunk_len));

            let (config_str, port_map) = self.generate_multi_port_config(chunk, &settings)?;
            let config_path = self.app_data_dir.join("inspector_config.json");
            std::fs::write(&config_path, config_str)?;

            let binary_path = self.locate_sing_box(&app)?;
            let mut cmd = std::process::Command::new(&binary_path);
            cmd.arg("run").arg("-c").arg(&config_path)
                .creation_flags(0x08000000); // CREATE_NO_WINDOW

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    emit_progress(tested_count, total_nodes, &format!("Failed to start inspector: {}", e));
                    continue;
                }
            };

            // wait for sing-box to start
            sleep(Duration::from_millis(1000)).await;

            let mut tasks = vec![];
            for (node_id, port) in port_map {
                let app_handle = app.clone();
                let task = tokio::spawn(async move {
                    let res = Self::test_node_latency_and_country(port).await;
                    (node_id, res)
                });
                tasks.push(task);
            }

            let mut completed_in_chunk = 0;
            for task in tasks {
                if let Ok((node_id, res)) = task.await {
                    if let Some(node) = chunk.iter_mut().find(|n| n.id == node_id) {
                        if let Ok((latency, cc, country)) = res {
                            node.latency_ms = Some(latency as i64);
                            if cc != "UN" && !cc.is_empty() && cc.len() == 2 {
                                node.country_code = cc;
                                if !country.is_empty() {
                                    node.country_name = country;
                                }
                            }
                        } else {
                            node.latency_ms = Some(9999);
                        }
                    }
                }
                completed_in_chunk += 1;
                emit_progress(tested_count + completed_in_chunk, total_nodes, "Testing latency & country...");
            }

            let _ = child.kill();
            let _ = child.wait();

            tested_count += chunk_len;
        }

        // Sort by latency
        results.sort_by(|a, b| {
            let l1 = a.latency_ms.unwrap_or(9999);
            let l2 = b.latency_ms.unwrap_or(9999);
            l1.cmp(&l2)
        });

        // Speedtest top 10
        let top_n = std::cmp::min(10, results.len());
        
        if top_n > 0 {
            emit_progress(tested_count, total_nodes, &format!("Preparing speedtest for top {} nodes...", top_n));
            let top_nodes: Vec<UnifiedNode> = results.iter().take(top_n).cloned().collect();
            
            let (config_str, port_map) = self.generate_multi_port_config(&top_nodes, &settings)?;
            let config_path = self.app_data_dir.join("inspector_config.json");
            std::fs::write(&config_path, config_str)?;

            let binary_path = self.locate_sing_box(&app)?;
            let mut cmd = std::process::Command::new(&binary_path);
            cmd.arg("run").arg("-c").arg(&config_path)
                .creation_flags(0x08000000); // CREATE_NO_WINDOW

            let mut child = cmd.spawn()?;
            sleep(Duration::from_millis(1000)).await;

            let mut speed_tasks = vec![];
            for (node_id, port) in port_map {
                let task = tokio::spawn(async move {
                    let res = Self::test_node_speed(port).await;
                    (node_id, res)
                });
                speed_tasks.push(task);
            }

            let mut speed_completed = 0;
            for task in speed_tasks {
                if let Ok((node_id, res)) = task.await {
                    if let Some(node) = results.iter_mut().find(|n| n.id == node_id) {
                        if let Ok(speed) = res {
                            node.speed_bps = Some(speed);
                        }
                    }
                }
                speed_completed += 1;
                emit_progress(tested_count, total_nodes, &format!("Speedtesting: {}/{}", speed_completed, top_n));
            }

            let _ = child.kill();
            let _ = child.wait();
        }

        // Rename nodes based on their country and speed without destroying the original provider name
        for node in results.iter_mut() {
            if let Some(lat) = node.latency_ms {
                if lat < 9999 {
                    let speed_mbps = node.speed_bps.unwrap_or(0) / 125_000;
                    let emoji = Self::country_code_to_emoji(&node.country_code);
                    let clean_name = node.name.trim();

                    // Only prepend country tag if it isn't already present in the name
                    if !node.country_code.is_empty() && node.country_code != "UN" {
                        let upper_code = node.country_code.to_uppercase();
                        if !clean_name.contains(&emoji) && !clean_name.to_uppercase().contains(&upper_code) {
                            if speed_mbps > 0 {
                                node.name = format!("{} [{}] {} ({}ms·{}M)", emoji, upper_code, clean_name, lat, speed_mbps);
                            } else {
                                node.name = format!("{} [{}] {} ({}ms)", emoji, upper_code, clean_name, lat);
                            }
                            continue;
                        }
                    }

                    if speed_mbps > 0 {
                        node.name = format!("{} ({}ms·{}M)", clean_name, lat, speed_mbps);
                    }
                } else {
                    node.name = format!("{} (Timeout)", node.name);
                }
            }
        }

        emit_progress(results.len(), results.len(), "Inspection completed!");
        Ok(results)
    }

    fn generate_multi_port_config(&self, nodes: &[UnifiedNode], settings: &AppSettings) -> Result<(String, Vec<(String, u16)>)> {
        let adapter = SingBoxAdapter::new();
        let mut outbounds = vec![];
        let mut inbounds = vec![];
        let mut rules = vec![];
        let mut port_map = vec![];

        let start_port = 30000;


        for (i, node) in nodes.iter().enumerate() {
            let mut ob = match adapter.build_outbound(node) {
                Ok(o) => o,
                Err(_) => continue,
            };

            let tag = format!("out-{}", i);
            ob["tag"] = json!(tag);
            outbounds.push(ob);

            let port = start_port + i as u16;
            let in_tag = format!("in-{}", i);
            inbounds.push(json!({
                "type": "mixed",
                "tag": in_tag,
                "listen": "127.0.0.1",
                "listen_port": port
            }));

            rules.push(json!({
                "inbound": in_tag,
                "outbound": tag
            }));

            port_map.push((node.id.clone(), port));
        }

        let config = json!({
            "inbounds": inbounds,
            "outbounds": outbounds,
            "route": {
                "rules": rules
            }
        });

        Ok((serde_json::to_string_pretty(&config)?, port_map))
    }

    async fn test_node_latency_and_country(port: u16) -> Result<(u64, String, String)> {
        let proxy_url = format!("socks5://127.0.0.1:{}", port);
        let proxy = reqwest::Proxy::all(&proxy_url)?;
        let client = reqwest::Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_secs(6))
            .build()?;

        // 1. Measure real latency via standard generate_204 endpoint
        let start = Instant::now();
        let _ = client.get("http://cp.cloudflare.com/generate_204").send().await;
        let latency = start.elapsed().as_millis() as u64;

        // 2. Query accurate GeoIP via ip-api.com
        let mut cc = String::new();
        let mut country = String::new();

        if let Ok(res) = client.get("http://ip-api.com/json").send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if json.get("status").and_then(|s| s.as_str()) == Some("success") {
                    if let Some(code) = json.get("countryCode").and_then(|c| c.as_str()) {
                        cc = code.to_uppercase();
                    }
                    if let Some(name) = json.get("country").and_then(|c| c.as_str()) {
                        country = name.to_string();
                    }
                }
            }
        }

        // Fallback to ipwho.is if needed
        if cc.is_empty() {
            if let Ok(res) = client.get("http://ipwho.is/").send().await {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    if json.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
                        if let Some(code) = json.get("country_code").and_then(|c| c.as_str()) {
                            cc = code.to_uppercase();
                        }
                        if let Some(name) = json.get("country").and_then(|c| c.as_str()) {
                            country = name.to_string();
                        }
                    }
                }
            }
        }

        if cc.is_empty() {
            cc = "UN".to_string();
        }

        Ok((latency, cc, country))
    }

    async fn test_node_speed(port: u16) -> Result<u64> {
        let proxy_url = format!("socks5://127.0.0.1:{}", port);
        let proxy = reqwest::Proxy::all(&proxy_url)?;
        let client = reqwest::Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_secs(10))
            .build()?;

        let start = Instant::now();
        // Download 2MB
        let res = client.get("https://speed.cloudflare.com/__down?bytes=2000000").send().await?;
        let bytes = res.bytes().await?;
        let elapsed = start.elapsed().as_secs_f64();
        
        if elapsed > 0.0 {
            let bps = (bytes.len() as f64 / elapsed) as u64;
            Ok(bps)
        } else {
            Ok(0)
        }
    }

    fn locate_sing_box(&self, app: &AppHandle) -> Result<PathBuf> {
        use tauri::Manager;
        let mut candidates = Vec::new();

        if let Ok(res_dir) = app.path().resource_dir() {
            candidates.push(res_dir.join("binaries").join("sing-box.exe"));
            candidates.push(res_dir.join("sing-box.exe"));
        }

        let exe_dir = std::env::current_exe()
            .map(|p| p.parent().unwrap_or(Path::new("")).to_path_buf())
            .unwrap_or_default();

        candidates.push(exe_dir.join("binaries").join("sing-box.exe"));
        candidates.push(exe_dir.join("sing-box.exe"));
        candidates.push(PathBuf::from("sing-box.exe"));

        for cand in candidates {
            if cand.exists() {
                return Ok(cand);
            }
        }
        bail!("sing-box.exe not found")
    }

    fn country_code_to_emoji(cc: &str) -> String {
        if cc.len() != 2 || cc == "UN" {
            return "🏳️‍🌈".to_string();
        }
        let cc = cc.to_uppercase();
        let mut emoji = String::new();
        for c in cc.chars() {
            let offset = c as u32 - 'A' as u32;
            if let Some(c) = std::char::from_u32(0x1F1E6 + offset) {
                emoji.push(c);
            }
        }
        emoji
    }
}
