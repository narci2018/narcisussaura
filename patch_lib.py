import re

with open('src-tauri/src/lib.rs', 'r', encoding='utf-8') as f:
    content = f.read()

new_cmd = '''
#[tauri::command]
async fn connect_smart_group(
    node_ids: Vec<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut nodes = Vec::new();
    for id in node_ids {
        if let Some(node) = state.node_manager.get_by_id(&id) {
            nodes.push(node);
        }
    }
    
    if nodes.is_empty() {
        return Err("No valid nodes found for smart group".to_string());
    }
    
    let settings = state.settings.read().clone();
    state.connection_manager.connect_smart_group(nodes, settings, app).await
}
'''

content += new_cmd
content = content.replace('generate_handler![', 'generate_handler![connect_smart_group, ')

with open('src-tauri/src/lib.rs', 'w', encoding='utf-8') as f:
    f.write(content)
