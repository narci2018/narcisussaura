import re

with open('src-tauri/src/lib.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Add InspectorManager import
content = content.replace(
    'use crate::managers::{ChainManager, ConnectionManager, NodeManager, SpeedTestManager, SubscriptionManager};',
    'use crate::managers::{ChainManager, ConnectionManager, NodeManager, SpeedTestManager, SubscriptionManager, InspectorManager};'
)

# Add inspector_manager to AppState
content = content.replace(
    '    pub chain_manager: ChainManager,\n    pub settings: Arc<RwLock<AppSettings>>,\n}',
    '    pub chain_manager: ChainManager,\n    pub inspector_manager: InspectorManager,\n    pub settings: Arc<RwLock<AppSettings>>,\n}'
)

# Add start_deep_inspection command
command_code = '''
#[tauri::command]
async fn start_deep_inspection(nodes: Vec<UnifiedNode>, state: State<'_, AppState>, app: AppHandle) -> Result<Vec<UnifiedNode>, String> {
    let settings = state.settings.read().clone();
    state.inspector_manager.start_deep_inspection(nodes, settings, app).await.map_err(|e| e.to_string())
}
'''
content = content.replace('#[tauri::command]\nasync fn get_machine_id', command_code + '\n#[tauri::command]\nasync fn get_machine_id')

# Register start_deep_inspection in generate_context
content = content.replace(
    '            connect,\n',
    '            connect,\n            start_deep_inspection,\n'
)

with open('src-tauri/src/lib.rs', 'w', encoding='utf-8') as f:
    f.write(content)
