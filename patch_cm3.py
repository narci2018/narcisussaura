import re

with open('src-tauri/src/managers/connection_manager.rs', 'r', encoding='utf-8', errors='ignore') as f:
    content = f.read()

replacement = '''
    pub async fn connect_smart_group(
        &self,
        nodes: Vec<UnifiedNode>,
        settings: AppSettings,
        app: AppHandle,
    ) -> Result<(), String> {
        if nodes.is_empty() {
            return Err("Smart group requires at least 1 node.".to_string());
        }

        let _ = self.disconnect(app.clone()).await;
        self.ensure_rules_deployed(&app);

        *self.status.lock() = ConnectionStatus::Connecting;
        *self.connected_node.lock() = Some(nodes[0].clone()); 
        *self.connected_chain.lock() = Some("smart-group".to_string());
        let _ = app.emit("core:status-changed", ConnectionStatus::Connecting);

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let binary_path = self.locate_sing_box(&app)?;
        let adapter = SingBoxAdapter::new();
        let config_str = adapter
            .generate_config_for_urltest(&nodes, &settings, &self.app_data_dir)
            .map_err(|e| format!("Failed to generate urltest config: {}", e))?;

        let config_path = self.app_data_dir.join("current_config.json");
        std::fs::write(&config_path, config_str).map_err(|e| format!("Failed to write core config file: {}", e))?;

        let log_file_path = self.app_data_dir.join("singbox.log");
        let log_out = std::fs::File::create(&log_file_path).unwrap();
        let log_err = log_out.try_clone().unwrap();

        let mut cmd = Command::new(&binary_path);
        cmd.arg("run").arg("-c").arg(&config_path)
            .stdout(std::process::Stdio::from(log_out))
            .stderr(std::process::Stdio::from(log_err))
            .creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd.spawn().map_err(|e| format!("Failed to start sing-box process: {}", e))?;

        if let Some(ref guard) = self.job_guard {
            let _ = guard.assign_process(&child);
        }

        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if let Ok(Some(exit_status)) = child.try_wait() {
            let _ = WindowsProxy::disable_proxy();
            *self.status.lock() = ConnectionStatus::Error;
            *self.connected_node.lock() = None;
            *self.connected_chain.lock() = None;
            let _ = app.emit("core:status-changed", ConnectionStatus::Error);
            return Err("Smart group core startup failed".into());
        }

        *self.process.lock() = Some(child);

        if settings.routing_mode == "global" || settings.routing_mode == "rule" {
            let _ = WindowsProxy::enable_proxy(settings.mixed_port);
        }

        *self.status.lock() = ConnectionStatus::Connected;
        let _ = app.emit("core:status-changed", ConnectionStatus::Connected);

        Ok(())
    }
}
'''

content = re.sub(r'\}\s*$', replacement, content)
with open('src-tauri/src/managers/connection_manager.rs', 'w', encoding='utf-8') as f:
    f.write(content)
