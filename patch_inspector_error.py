import re

with open('src-tauri/src/managers/inspector_manager.rs', 'r', encoding='utf-8') as f:
    content = f.read()

replacement = '''
        for (i, node) in nodes.iter().enumerate() {
            let mut ob = match adapter.build_outbound(node) {
                Ok(o) => o,
                Err(_) => continue,
            };
'''
content = content.replace(
'''        for (i, node) in nodes.iter().enumerate() {
            let mut ob = adapter.build_outbound(node)?;''',
replacement
)

with open('src-tauri/src/managers/inspector_manager.rs', 'w', encoding='utf-8') as f:
    f.write(content)
