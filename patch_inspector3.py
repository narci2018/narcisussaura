import re

with open('src-tauri/src/managers/inspector_manager.rs', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('let total_nodes = total_nodes;', 'let total_nodes = results.len();')
content = content.replace('emit_progress(0, total_nodes, "Preparing inspector...");', 'emit_progress(0, results.len(), "Preparing inspector...");')
content = content.replace('let top_n = std::cmp::min(10, total_nodes);', 'let top_n = std::cmp::min(10, results.len());')
content = content.replace('emit_progress(total_nodes, total_nodes, "Inspection completed!");', 'emit_progress(results.len(), results.len(), "Inspection completed!");')

with open('src-tauri/src/managers/inspector_manager.rs', 'w', encoding='utf-8') as f:
    f.write(content)
