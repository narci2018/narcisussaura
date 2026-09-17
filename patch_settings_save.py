import re

with open('src-tauri/src/lib.rs', 'r', encoding='utf-8') as f:
    content = f.read()

replacement1 = '''
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
'''
content = re.sub(r'async fn save_settings\(new_settings: AppSettings, state: State<\'_, AppState>\) -> Result<\(\), String> \{.*?\n\}', replacement1.strip(), content, flags=re.DOTALL)

replacement2 = '''
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
'''
content = content.replace(
'''            let inspector_manager = InspectorManager::new(app_data_dir.clone());
            let settings = Arc::new(RwLock::new(AppSettings::default()));''',
replacement2)

with open('src-tauri/src/lib.rs', 'w', encoding='utf-8') as f:
    f.write(content)
