import re

with open('src-tauri/src/managers/inspector_manager.rs', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('let chunk_size = 20;', 'let chunk_size = 20;\n        let total_nodes = results.len();')
content = content.replace('results.len()', 'total_nodes')

content = content.replace('if let Some(char) = std::char::from_u32(0x1F1E6 + offset) {\n                emoji.push(char.unwrap());\n            }', 'if let Some(c) = std::char::from_u32(0x1F1E6 + offset) {\n                emoji.push(c);\n            }')

with open('src-tauri/src/managers/inspector_manager.rs', 'w', encoding='utf-8') as f:
    f.write(content)
