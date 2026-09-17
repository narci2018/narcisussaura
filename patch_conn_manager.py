import re

with open('src-tauri/src/managers/connection_manager.rs', 'r', encoding='utf-8') as f:
    content = f.read()

replacement = '''
        *self.process.lock() = Some(child);
        *self.connect_time.lock() = Some(Instant::now());

        if settings.routing_mode == "global" || settings.routing_mode == "rule" {
            let _ = WindowsProxy::enable_proxy(settings.mixed_port);
        }

        *self.status.lock() = ConnectionStatus::Connected;
        let _ = app.emit("core:status-changed", ConnectionStatus::Connected);
        self.start_traffic_monitor(app.clone(), settings.clash_api_port);

        Ok(())
'''

content = content.replace(
'''        *self.process.lock() = Some(child);

        if settings.routing_mode == "global" || settings.routing_mode == "rule" {
            let _ = WindowsProxy::enable_proxy(settings.mixed_port);
        }

        *self.status.lock() = ConnectionStatus::Connected;
        let _ = app.emit("core:status-changed", ConnectionStatus::Connected);

        Ok(())''',
replacement)

with open('src-tauri/src/managers/connection_manager.rs', 'w', encoding='utf-8') as f:
    f.write(content)
