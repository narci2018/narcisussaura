import re

with open('src-tauri/src/managers/inspector_manager.rs', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('use crate::adapters::SingBoxAdapter;', 'use crate::core::SingBoxAdapter;\nuse std::os::windows::process::CommandExt;')
content = content.replace('use serde_json::{json, Value};', 'use serde_json::json;')
content = content.replace('if let char = std::char::from_u32(0x1F1E6 + offset) {', 'if let Some(char) = std::char::from_u32(0x1F1E6 + offset) {')

with open('src-tauri/src/managers/inspector_manager.rs', 'w', encoding='utf-8') as f:
    f.write(content)
