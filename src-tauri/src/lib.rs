pub mod core;
pub mod managers;
pub mod models;
pub mod platform;

use crate::managers::{ChainManager, ConnectionManager, NodeManager, SpeedTestManager, SubscriptionManager, InspectorManager};
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
async fn get_machine_id() -> Result<String, String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        Ok("mobile-client".to_string())
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        machine_uid::get().map_err(|e| e.to_string())
    }
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

    let relay_node = match relay_node_id.as_deref() {
        Some("none") | Some("direct") => None,
        Some("auto") => state.node_manager.get_best_relay_node(),
        Some(id) if !id.trim().is_empty() => state.node_manager.get_by_id(id).or_else(|| state.node_manager.get_best_relay_node()),
        _ => None,
    };

    let settings = state.settings.read().clone();
    state.connection_manager.connect(node, relay_node, settings, app).await
}

#[tauri::command]
async fn get_relay_candidates(state: State<'_, AppState>) -> Result<Vec<UnifiedNode>, String> {
    Ok(state.node_manager.get_relay_candidates())
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
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_maximize(window: Window) -> Result<bool, String> {
    let is_max = window.is_maximized().map_err(|e| e.to_string())?;
    if is_max {
        window.unmaximize().map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        window.maximize().map_err(|e| e.to_string())?;
        Ok(true)
    }
}

#[tauri::command]
async fn is_window_maximized(window: Window) -> Result<bool, String> {
    window.is_maximized().map_err(|e| e.to_string())
}

#[tauri::command]
async fn close_window(window: Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

#[tauri::command]
async fn fetch_megav_nodes(state: State<'_, AppState>) -> Result<Vec<UnifiedNode>, String> {
    crate::managers::SpecialSources::fetch_megav_nodes(&state.node_manager).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn fetch_vpngate_nodes(state: State<'_, AppState>) -> Result<Vec<UnifiedNode>, String> {
    crate::managers::SpecialSources::fetch_vpngate_nodes(&state.node_manager).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn fetch_psiphon_nodes(state: State<'_, AppState>) -> Result<Vec<UnifiedNode>, String> {
    Ok(crate::managers::SpecialSources::fetch_psiphon_nodes(&state.node_manager))
}

#[tauri::command]
async fn fetch_residential_nodes(url: Option<String>, state: State<'_, AppState>) -> Result<Vec<UnifiedNode>, String> {
    crate::managers::SpecialSources::fetch_residential_nodes(&state.node_manager, url).await.map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let _ = std::fs::create_dir_all(&app_data_dir);

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
            get_relay_candidates,
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
    let mut nodes = Vec::new();
    for id in node_ids {
        if let Some(node) = state.node_manager.get_by_id(&id) {
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
        return Err("当前没有可用的常规订阅节点供智能最优选路连接".to_string());
    }
    
    let settings = state.settings.read().clone();
    state.connection_manager.connect_smart_group(nodes, settings, app).await
}
