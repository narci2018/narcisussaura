pub mod core;
pub mod managers;
pub mod models;
pub mod platform;

use crate::managers::{liveness, relay_selector, vpngate_sources, ChainManager, ConnectionManager, NodeManager, SpeedTestManager, SubscriptionManager, InspectorManager};
use crate::models::{AppSettings, ConnectionStatus, ProxyChain, Subscription, UnifiedNode};
use parking_lot::RwLock;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State, Window};

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};

pub struct AppState {
    pub node_manager: NodeManager,
    pub subscription_manager: SubscriptionManager,
    pub connection_manager: ConnectionManager,
    pub chain_manager: ChainManager,
    pub inspector_manager: InspectorManager,
    pub settings: Arc<RwLock<AppSettings>>,
}


#[tauri::command]
async fn start_deep_inspection(nodes: Vec<UnifiedNode>, state: State<'_, AppState>, app: AppHandle) -> Result<Vec<UnifiedNode>, String> {
    let settings = state.settings.read().clone();
    state.inspector_manager.start_deep_inspection(nodes, settings, app).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_crash_report(app: AppHandle) -> Option<String> {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        let dir = app.path().app_data_dir().ok()?;
        crate::platform::android::read_crash_report(&dir)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        None
    }
}

/// Concatenate every diagnostic log the app owns so the user can copy the
/// whole thing in one tap (the in-app views cannot scroll to the end of long
/// logs on some devices). Tail-bounded to keep the clipboard usable.
#[tauri::command]
async fn get_full_logs(app: AppHandle) -> Result<String, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {}", e))?;
    let tail = |path: std::path::PathBuf, max_lines: usize| -> String {
        match std::fs::read_to_string(&path) {
            Ok(s) => {
                let lines: Vec<&str> = s.lines().collect();
                let skip = lines.len().saturating_sub(max_lines);
                let mut out = String::new();
                if skip > 0 {
                    out.push_str(&format!("... (前 {} 行省略)\n", skip));
                }
                out.push_str(lines[skip..].join("\n").as_str());
                out
            }
            Err(_) => "(文件不存在)".to_string(),
        }
    };
    let mut parts = vec![
        format!("app版本: v{}", env!("CARGO_PKG_VERSION")),
        format!("===== singbox.log =====\n{}", tail(dir.join("singbox.log"), 300)),
        format!("===== tunrelay.log =====\n{}", tail(dir.join("tunrelay.log"), 400)),
    ];
    // 住宅/VPNGate 与后台测活走 mihomo:没有这两类日志就只能靠猜
    let mihomo = tail(dir.join("mihomo.log"), 300);
    if mihomo != "(文件不存在)" {
        parts.push(format!("===== mihomo.log =====\n{}", mihomo));
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut lanes: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("lane_") && n.ends_with(".log"))
                    .unwrap_or(false)
            })
            .collect();
        lanes.sort();
        for path in lanes {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("lane.log")
                .to_string();
            let body = tail(path, 120);
            if !body.trim().is_empty() {
                parts.push(format!("===== {} =====\n{}", name, body));
            }
        }
    }
    for name in ["crash_log", "panic_log"] {
        let s = tail(dir.join(name), 60);
        if s != "(文件不存在)" && !s.trim().is_empty() {
            parts.push(format!("===== {} =====\n{}", name, s));
        }
    }
    Ok(parts.join("\n\n"))
}

#[tauri::command]
#[allow(unused_variables)]
async fn get_machine_id(app: AppHandle) -> Result<String, String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri::Manager;
        let app_data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let id_file = app_data_dir.join("machine_id");
        for _ in 0..10 {
            if let Ok(id) = std::fs::read_to_string(&id_file) {
                let trimmed = id.trim().to_string();
                if !trimmed.is_empty() {
                    return Ok(trimmed);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        Err("machine_id not found: VpnInitProvider has not written the file yet".to_string())
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        machine_uid::get().map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn request_auth(machine_id: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let res = client
        .post("https://auth.lkhotrich.kdns.fr/api/auth")
        .json(&serde_json::json!({ "machine_id": machine_id }))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    
    let body = res.text().await.map_err(|e| format!("Failed to read response: {}", e))?;
    Ok(body)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn rand_u32() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    (t.subsec_nanos() ^ (t.as_secs() as u32).wrapping_mul(2654435761)) as u32
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn rand_u16() -> u16 {
    (rand_u32() & 0xFFFF) as u16
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn rand_u48() -> u64 {
    let hi = rand_u32() as u64;
    let lo = rand_u32() as u64;
    ((hi << 16) | lo) & 0xFFFFFFFFFFFF
}

#[tauri::command]
async fn get_connection_status(state: State<'_, AppState>) -> Result<ConnectionStatus, String> {
    Ok(state.connection_manager.get_status())
}

#[tauri::command]
async fn get_connected_node(state: State<'_, AppState>) -> Result<Option<UnifiedNode>, String> {
    Ok(state.connection_manager.get_connected_node())
}

#[tauri::command]
async fn connect(
    node_id: String,
    relay_node_id: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let node = state
        .node_manager
        .get_by_id(&node_id)
        .ok_or_else(|| "Node not found".to_string())?;

    let preferred = state.settings.read().preferred_relay_id.clone();
    let relay_node = match relay_node_id.as_deref() {
        Some("none") | Some("direct") => None,
        Some("auto") => state.node_manager.get_best_relay_node(preferred.as_deref()),
        Some(id) if !id.trim().is_empty() => state
            .node_manager
            .get_by_id(id)
            .or_else(|| state.node_manager.get_best_relay_node(preferred.as_deref())),
        _ => None,
    };

    let settings = state.settings.read().clone();
    state.connection_manager.connect(node, relay_node, settings, app).await
}

#[tauri::command]
async fn get_relay_candidates(state: State<'_, AppState>) -> Result<Vec<UnifiedNode>, String> {
    Ok(state.node_manager.get_relay_candidates())
}

/// Remember who won the relay ranking and tell the UI. Shared by the startup
/// job and the explicit command, because both must leave the same state behind.
/// Returns the winner's id so callers can hop the next request through it.
fn store_preferred_relay(
    app: &AppHandle,
    state: &AppState,
    ranking: &relay_selector::RelayRanking,
) -> Option<String> {
    if let Some(id) = &ranking.preferred_id {
        let snapshot = {
            let mut settings = state.settings.write();
            settings.preferred_relay_id = Some(id.clone());
            settings.clone()
        };
        if let Ok(dir) = app.path().app_data_dir() {
            if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
                let _ = std::fs::write(dir.join("settings.json"), json);
            }
        }
    }
    relay_selector::emit_ranking(app, ranking);
    ranking.preferred_id.clone()
}

/// Dial the relay shortlist for real and remember the winner, so that every
/// later "auto" chain hop is a measured choice instead of a guess. The same
/// routine runs on its own a few seconds after boot (see `setup`); this command
/// is the manual "重新实测" button in the relay bar.
#[tauri::command]
async fn rank_relays(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<relay_selector::RelayRanking, String> {
    let candidates = state.node_manager.get_relay_candidates();
    let ranking = relay_selector::select_preferred_relay(
        &app,
        &state.node_manager,
        &state.connection_manager,
        candidates,
    )
    .await?;
    store_preferred_relay(&app, &state, &ranking);
    Ok(ranking)
}

#[tauri::command]
async fn disconnect(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.connection_manager.disconnect(app).await
}

#[tauri::command]
async fn get_nodes(state: State<'_, AppState>) -> Result<Vec<UnifiedNode>, String> {
    Ok(state.node_manager.get_all())
}

#[tauri::command]
async fn add_node(node: UnifiedNode, state: State<'_, AppState>) -> Result<UnifiedNode, String> {
    state.node_manager.add_node(node)
}

#[tauri::command]
async fn update_node(node: UnifiedNode, state: State<'_, AppState>) -> Result<(), String> {
    state.node_manager.update_node(node)
}

#[tauri::command]
async fn delete_node(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.node_manager.delete_node(&id)
}

#[tauri::command]
async fn toggle_favorite(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    state.node_manager.toggle_favorite(&id)
}

#[tauri::command]
async fn import_share_link(link: String, state: State<'_, AppState>) -> Result<UnifiedNode, String> {
    let node = state.node_manager.parse_share_link(&link)?;
    state.node_manager.add_node(node)
}

#[tauri::command]
async fn test_node_latency(id: String, state: State<'_, AppState>) -> Result<i64, String> {
    let node = state
        .node_manager
        .get_by_id(&id)
        .ok_or_else(|| "Node not found".to_string())?;

    let latency = SpeedTestManager::ping_node(&node.protocol, &node.address, node.port).await;
    state.node_manager.update_latency(&id, Some(latency));
    let _ = state.node_manager.save();
    Ok(latency)
}

#[tauri::command]
async fn test_node_speed(id: String, state: State<'_, AppState>) -> Result<Option<u64>, String> {
    let node = state
        .node_manager
        .get_by_id(&id)
        .ok_or_else(|| "Node not found".to_string())?;

    let settings = state.settings.read().clone();
    let connected = state.connection_manager.get_connected_node();
    let is_connected = connected.as_ref().map(|n| n.id.as_str()) == Some(&id);

    let lat = SpeedTestManager::ping_node(&node.protocol, &node.address, node.port).await;
    state.node_manager.update_latency(&id, Some(lat));

    if lat < 0 || lat >= 1000 {
        state.node_manager.update_speed(&id, None);
        let _ = state.node_manager.save();
        return Ok(None);
    }

    let speed = if is_connected {
        if let Some(s) = SpeedTestManager::measure_download_speed(settings.mixed_port).await {
            Some(s)
        } else {
            SpeedTestManager::probe_node_speed(&node.protocol, &node.address, node.port, lat).await
        }
    } else {
        SpeedTestManager::probe_node_speed(&node.protocol, &node.address, node.port, lat).await
    };

    state.node_manager.update_speed(&id, speed);
    let _ = state.node_manager.save();
    Ok(speed)
}

#[tauri::command]
async fn test_all_nodes(state: State<'_, AppState>) -> Result<Vec<UnifiedNode>, String> {
    let nodes = state.node_manager.get_all();
    let sem = Arc::new(tokio::sync::Semaphore::new(10));

    let mut tasks = Vec::new();
    for node in nodes {
        let sem_clone = Arc::clone(&sem);
        let id = node.id.clone();
        let proto = node.protocol.clone();
        let addr = node.address.clone();
        let port = node.port;

        tasks.push(tokio::spawn(async move {
            let _permit = sem_clone.acquire().await;
            let latency = SpeedTestManager::ping_node(&proto, &addr, port).await;
            (id, latency)
        }));
    }

    for task in tasks {
        if let Ok((id, latency)) = task.await {
            state.node_manager.update_latency(&id, Some(latency));
        }
    }
    let _ = state.node_manager.save();

    Ok(state.node_manager.get_all())
}

#[tauri::command]
async fn test_all_speeds(state: State<'_, AppState>) -> Result<Vec<UnifiedNode>, String> {
    let nodes = state.node_manager.get_all();
    let settings = state.settings.read().clone();
    let connected = state.connection_manager.get_connected_node();
    let sem = Arc::new(tokio::sync::Semaphore::new(6));

    let mut tasks = Vec::new();
    for node in nodes {
        let sem_clone = Arc::clone(&sem);
        let id = node.id.clone();
        let proto = node.protocol.clone();
        let addr = node.address.clone();
        let port = node.port;
        let is_connected = connected.as_ref().map(|n| n.id.as_str()) == Some(&id);
        let mixed_port = settings.mixed_port;

        tasks.push(tokio::spawn(async move {
            let _permit = sem_clone.acquire().await;
            let lat = SpeedTestManager::ping_node(&proto, &addr, port).await;
            if lat < 0 || lat >= 1000 {
                return (id, lat, None);
            }

            let speed = if is_connected {
                if let Some(s) = SpeedTestManager::measure_download_speed(mixed_port).await {
                    Some(s)
                } else {
                    SpeedTestManager::probe_node_speed(&proto, &addr, port, lat).await
                }
            } else {
                SpeedTestManager::probe_node_speed(&proto, &addr, port, lat).await
            };

            (id, lat, speed)
        }));
    }

    for task in tasks {
        if let Ok((id, lat, speed)) = task.await {
            state.node_manager.update_latency(&id, Some(lat));
            state.node_manager.update_speed(&id, speed);
        }
    }
    let _ = state.node_manager.save();

    Ok(state.node_manager.get_all())
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.settings.read().clone())
}

#[tauri::command]
async fn save_settings(new_settings: AppSettings, state: State<'_, AppState>, app: tauri::AppHandle) -> Result<(), String> {
    // preferred_relay_id is written by the ranking job, never by the settings
    // form — a save that carries no preference must not erase the measured one.
    let mut new_settings = new_settings;
    if new_settings.preferred_relay_id.is_none() {
        new_settings.preferred_relay_id = state.settings.read().preferred_relay_id.clone();
    }
    *state.settings.write() = new_settings.clone();
    
    // Save to disk
    if let Ok(app_data_dir) = app.path().app_data_dir() {
        let settings_path = app_data_dir.join("settings.json");
        if let Ok(json_str) = serde_json::to_string_pretty(&new_settings) {
            let _ = std::fs::write(settings_path, json_str);
        }
    }
    
    Ok(())
}

#[tauri::command]
async fn get_subscriptions(state: State<'_, AppState>) -> Result<Vec<Subscription>, String> {
    Ok(state.subscription_manager.get_all())
}

#[tauri::command]
async fn restore_default_subscriptions(state: State<'_, AppState>) -> Result<Vec<Subscription>, String> {
    state.subscription_manager.restore_default_subscriptions()
}

#[tauri::command]
async fn add_subscription(
    name: String,
    url: String,
    state: State<'_, AppState>,
) -> Result<Subscription, String> {
    state.subscription_manager.add_subscription(name, url)
}

#[tauri::command]
async fn edit_subscription(
    id: String,
    name: String,
    url: String,
    state: State<'_, AppState>,
) -> Result<Subscription, String> {
    state.subscription_manager.edit_subscription(&id, name, url)
}

#[tauri::command]
async fn delete_subscription(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.subscription_manager.delete_subscription(&id)
}

#[tauri::command]
async fn update_subscription(
    id: String,
    use_proxy: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Subscription, String> {
    let proxy_url = if use_proxy.unwrap_or(false) {
        if state.connection_manager.get_status() == ConnectionStatus::Connected {
            Some(format!("http://127.0.0.1:{}", state.settings.read().mixed_port))
        } else {
            None
        }
    } else {
        None
    };
    state.subscription_manager.update_subscription(&id, proxy_url.as_deref()).await
}

#[tauri::command]
async fn update_all_subscriptions(
    use_proxy: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<Subscription>, String> {
    let proxy_url = if use_proxy.unwrap_or(false) {
        if state.connection_manager.get_status() == ConnectionStatus::Connected {
            Some(format!("http://127.0.0.1:{}", state.settings.read().mixed_port))
        } else {
            None
        }
    } else {
        None
    };
    state.subscription_manager.update_all_subscriptions(proxy_url.as_deref()).await
}

#[tauri::command]
async fn minimize_window(window: Window) -> Result<(), String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        window.hide().map_err(|e| e.to_string())
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = window;
        Ok(())
    }
}

#[tauri::command]
async fn toggle_maximize(window: Window) -> Result<bool, String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let is_max = window.is_maximized().map_err(|e| e.to_string())?;
        if is_max {
            window.unmaximize().map_err(|e| e.to_string())?;
            Ok(false)
        } else {
            window.maximize().map_err(|e| e.to_string())?;
            Ok(true)
        }
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = window;
        Ok(false)
    }
}

#[tauri::command]
async fn is_window_maximized(window: Window) -> Result<bool, String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        window.is_maximized().map_err(|e| e.to_string())
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = window;
        Ok(false)
    }
}

#[tauri::command]
async fn close_window(window: Window) -> Result<(), String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        window.close().map_err(|e| e.to_string())
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = window;
        Ok(())
    }
}

#[tauri::command]
async fn fetch_megav_nodes(state: State<'_, AppState>) -> Result<Vec<UnifiedNode>, String> {
    crate::managers::SpecialSources::fetch_megav_nodes(&state.node_manager).await.map_err(|e| e.to_string())
}

/// Rebuild the VPNGate list from every source (mirror direct, official endpoints
/// through the preferred relay) and publish it.
///
/// Collecting a list and measuring it are separate commands now: a sweep dials
/// every server in it, which is a phone's worth of work nobody asked for just
/// because a tab was opened.
#[tauri::command]
async fn fetch_vpngate_nodes(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<UnifiedNode>, String> {
    let preferred = state.settings.read().preferred_relay_id.clone();
    vpngate_sources::sync(
        &app,
        &state.node_manager,
        &state.connection_manager,
        preferred.as_deref(),
    )
    .await
}

#[tauri::command]
async fn fetch_psiphon_nodes(state: State<'_, AppState>) -> Result<Vec<UnifiedNode>, String> {
    Ok(crate::managers::SpecialSources::fetch_psiphon_nodes(&state.node_manager))
}

#[tauri::command]
async fn fetch_residential_nodes(
    url: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<UnifiedNode>, String> {
    crate::managers::SpecialSources::fetch_residential_nodes(&state.node_manager, url)
        .await
        .map_err(|e| e.to_string())
}

/// Measure a whole public list for real, through the preferred relay.
///
/// The lanes are claimed **here**, not inside the spawned task: a press that
/// starts nothing must come back to the panel as a reason, otherwise the button
/// appears to have measured a list that is still all 未测.
#[tauri::command]
async fn measure_group_nodes(
    app: AppHandle,
    group: String,
) -> Result<(), String> {
    let group = liveness::known_group(&group).ok_or("该名单不需要真连接测活")?;
    let guard = liveness::claim_pass(&[group])?;
    spawn_liveness(app, Some(group), guard);
    Ok(())
}

/// Measure one server the user pointed at. The answer is always a sentence the
/// card can show — see [`liveness::ProbeOutcome`].
#[tauri::command]
async fn measure_node(
    app: AppHandle,
    state: State<'_, AppState>,
    node_id: String,
) -> Result<liveness::ProbeOutcome, String> {
    let preferred = state.settings.read().preferred_relay_id.clone();
    liveness::measure_one(
        &app,
        &state.node_manager,
        &state.connection_manager,
        &node_id,
        preferred.as_deref(),
    )
    .await
}

/// Dial the listed public servers for real, in the background.
///
/// `scope` names one list; `None` covers every public list. Only a button press
/// gets here, and it arrives with the lanes already claimed — see
/// [`measure_group_nodes`] for why the claim happens on the caller's side.
fn spawn_liveness(app: AppHandle, scope: Option<&'static str>, guard: liveness::PassGuard) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let preferred = state.settings.read().preferred_relay_id.clone();
        liveness::run_pass(
            &app,
            &state.node_manager,
            &state.connection_manager,
            preferred.as_deref(),
            scope,
            guard,
        )
        .await;
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let _ = std::fs::create_dir_all(&app_data_dir);

            // Android has no adb access for us, so a Rust panic must leave a
            // readable trace behind: write it to panic_log, surfaced in the UI
            // on the next launch via get_crash_report.
            #[cfg(target_os = "android")]
            crate::platform::android::install_panic_hook(&app_data_dir);

            if let Some(parent) = app_data_dir.parent() {
                let old_dir = parent.join("com.auravpn.client");
                if old_dir.exists() {
                    let sub_target = app_data_dir.join("subscriptions.json");
                    if !sub_target.exists() {
                        let _ = std::fs::copy(old_dir.join("subscriptions.json"), &sub_target);
                    }
                    let node_target = app_data_dir.join("nodes.json");
                    if !node_target.exists() {
                        let _ = std::fs::copy(old_dir.join("nodes.json"), &node_target);
                    }
                    let set_target = app_data_dir.join("settings.json");
                    if !set_target.exists() {
                        let _ = std::fs::copy(old_dir.join("settings.json"), &set_target);
                    }
                }
            }

            let node_manager = NodeManager::new(&app_data_dir);
            let subscription_manager = SubscriptionManager::new(&app_data_dir, node_manager.clone());
            let connection_manager = ConnectionManager::new(&app_data_dir);
            let chain_manager = ChainManager::new(&app_data_dir);

            let inspector_manager = InspectorManager::new(app_data_dir.clone());
            
            let mut initial_settings = AppSettings::default();
            let settings_path = app_data_dir.join("settings.json");
            if settings_path.exists() {
                if let Ok(json_str) = std::fs::read_to_string(&settings_path) {
                    if let Ok(parsed) = serde_json::from_str(&json_str) {
                        initial_settings = parsed;
                    }
                }
            }
            
            let settings = Arc::new(RwLock::new(initial_settings));


            app.manage(AppState {
                node_manager,
                subscription_manager,
                connection_manager,
                chain_manager,
                inspector_manager,
                settings,
            });

            // Pick the chain entry point before anything asks for it. Every
            // background job that follows (official VPNGate sources, liveness
            // probes) hops through this one relay, so it is measured first thing
            // after boot rather than guessed at connect time. The small delay
            // keeps the burst off the window's paint.
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    let state = handle.state::<AppState>();
                    let candidates = state.node_manager.get_relay_candidates();
                    let ranking = relay_selector::select_preferred_relay(
                        &handle,
                        &state.node_manager,
                        &state.connection_manager,
                        candidates,
                    )
                    .await;
                    let preferred = match ranking {
                        Ok(ranking) => store_preferred_relay(&handle, &state, &ranking),
                        Err(e) => {
                            log::warn!("startup relay ranking did not run: {}", e);
                            state.settings.read().preferred_relay_id.clone()
                        }
                    };
                    // The official VPNGate lists are only reachable through that
                    // relay, so they are pulled immediately behind it.
                    match vpngate_sources::sync(
                        &handle,
                        &state.node_manager,
                        &state.connection_manager,
                        preferred.as_deref(),
                    )
                    .await
                    {
                        Ok(nodes) => {
                            log::info!("startup: VPNGate list refreshed to {} servers", nodes.len())
                        }
                        Err(e) => log::warn!("startup: VPNGate list refresh failed: {}", e),
                    }
                    // Measuring those servers for real is deliberately not part of
                    // startup: a sweep dials every server in the list (~5s each,
                    // measured) and a phone should only pay that when the user
                    // presses 测活全部节点.
                    log::info!("startup: public node lists ready");
                });
            }

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let show_i = MenuItem::with_id(app, "show", "打开主界面", true, None::<&str>)?;
                let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

                let mut builder = TrayIconBuilder::new()
                    .menu(&menu)
                    .show_menu_on_left_click(false);

                if let Some(icon) = app.default_window_icon() {
                    builder = builder.icon(icon.clone());
                }

                builder
                    .on_menu_event(|app, event| {
                        match event.id.as_ref() {
                            "show" => {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.unminimize();
                                    let _ = window.set_focus();
                                }
                            }
                            "quit" => {
                                if let Some(state) = app.try_state::<AppState>() {
                                    state.connection_manager.shutdown();
                                }
                                app.exit(0);
                            }
                            _ => {}
                        }
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } = event {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app)?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![connect_smart_group, 
            get_machine_id,
            get_crash_report,
            get_full_logs,
            request_auth,
            get_connection_status,
            get_connected_node,
            connect,
            start_deep_inspection,
            disconnect,
            get_nodes,
            add_node,
            update_node,
            delete_node,
            toggle_favorite,
            import_share_link,
            test_node_latency,
            test_node_speed,
            test_all_nodes,
            test_all_speeds,
            get_subscriptions,
            restore_default_subscriptions,
            add_subscription,
            edit_subscription,
            delete_subscription,
            update_subscription,
            update_all_subscriptions,
            fetch_megav_nodes,
            fetch_vpngate_nodes,
            fetch_psiphon_nodes,
            fetch_residential_nodes,
            measure_group_nodes,
            measure_node,
            get_relay_candidates,
            rank_relays,
            get_settings,
            save_settings,
            get_chains,
            add_chain,
            update_chain,
            delete_chain,
            get_connected_chain,
            connect_chain,
            test_chain_latency,
            minimize_window,
            toggle_maximize,
            is_window_maximized,
            close_window
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    state.connection_manager.shutdown();
                }
            }
        });
}

#[tauri::command]
async fn get_chains(state: State<'_, AppState>) -> Result<Vec<ProxyChain>, String> {
    Ok(state.chain_manager.get_all())
}

#[tauri::command]
async fn add_chain(chain: ProxyChain, state: State<'_, AppState>) -> Result<ProxyChain, String> {
    state.chain_manager.add_chain(chain)
}

#[tauri::command]
async fn update_chain(chain: ProxyChain, state: State<'_, AppState>) -> Result<(), String> {
    state.chain_manager.update_chain(chain)
}

#[tauri::command]
async fn delete_chain(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.chain_manager.delete_chain(&id)
}

#[tauri::command]
async fn get_connected_chain(state: State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(state.connection_manager.get_connected_chain())
}

#[tauri::command]
async fn connect_chain(
    chain_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let chain = state
        .chain_manager
        .get_by_id(&chain_id)
        .ok_or_else(|| "Chain not found".to_string())?;

    let mut nodes = Vec::new();
    for nid in &chain.node_ids {
        if let Some(node) = state.node_manager.get_by_id(nid) {
            nodes.push(node);
        } else {
            return Err(format!("Chain node with ID '{}' was not found. Please re-check the chain configuration.", nid));
        }
    }

    let settings = state.settings.read().clone();
    state.connection_manager.connect_chain(chain_id, nodes, settings, app).await
}

#[tauri::command]
async fn test_chain_latency(chain_id: String, state: State<'_, AppState>) -> Result<i64, String> {
    let chain = state
        .chain_manager
        .get_by_id(&chain_id)
        .ok_or_else(|| "Chain not found".to_string())?;

    let mut nodes = Vec::new();
    for nid in &chain.node_ids {
        if let Some(node) = state.node_manager.get_by_id(nid) {
            nodes.push(node);
        } else {
            return Err(format!("Chain node '{}' not found", nid));
        }
    }

    let mut total_latency: i64 = 0;
    for node in &nodes {
        let lat = SpeedTestManager::ping_node(&node.protocol, &node.address, node.port).await;
        if lat < 0 {
            total_latency = -1;
            break;
        }
        total_latency += lat;
    }

    let _ = state.chain_manager.update_latency(&chain_id, Some(total_latency));
    Ok(total_latency)
}

#[tauri::command]
async fn connect_smart_group(
    node_ids: Vec<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let total = node_ids.len();
    let mut nodes = Vec::new();
    let mut found = 0usize;
    for id in node_ids {
        if let Some(node) = state.node_manager.get_by_id(&id) {
            found += 1;
            // Smart group strictly accepts regular subscription nodes only (Vless, Vmess, Trojan, Shadowsocks, Hysteria2)
            // Advanced proxy modes (Residential, VPNGate, Psiphon, MegaV, WARP, LocalProxy) are excluded
            let is_special = match node.protocol {
                models::ProtocolType::Openvpn
                | models::ProtocolType::Psiphon
                | models::ProtocolType::Masque
                | models::ProtocolType::Wireguard => true,
                _ => {
                    let g = node.group.to_uppercase();
                    g.contains("RESIDENTIAL")
                        || g.contains("VPNGATE")
                        || g.contains("PSIPHON")
                        || g.contains("MEGAV")
                        || g.contains("WARP")
                        || g.contains("LOCAL")
                        || g.contains("SEED")
                }
            };

            if !is_special {
                nodes.push(node);
            }
        }
    }

    if nodes.is_empty() {
        // Distinguish "backend lost the node list" from "frontend sent stale ids"
        // from "all filtered as special" — each has a different root cause, and
        // the user has no way to inspect internals without this breakdown.
        return Err(format!(
            "当前没有可用的常规订阅节点供智能最优选路连接（前端传入{}个ID；后端节点库找到{}个；被特殊模式排除{}个）",
            total,
            found,
            found.saturating_sub(nodes.len())
        ));
    }
    
    let settings = state.settings.read().clone();
    state.connection_manager.connect_smart_group(nodes, settings, app).await
}
