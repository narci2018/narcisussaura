import re

with open('src-tauri/src/lib.rs', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace(
    '            let chain_manager = ChainManager::new(&app_data_dir);\n            let settings = Arc::new(RwLock::new(AppSettings::default()));\n',
    '            let chain_manager = ChainManager::new(&app_data_dir);\n            let inspector_manager = InspectorManager::new(app_data_dir.clone());\n            let settings = Arc::new(RwLock::new(AppSettings::default()));\n'
)

content = content.replace(
    '                chain_manager,\n                settings,\n            });',
    '                chain_manager,\n                inspector_manager,\n                settings,\n            });'
)

with open('src-tauri/src/lib.rs', 'w', encoding='utf-8') as f:
    f.write(content)
