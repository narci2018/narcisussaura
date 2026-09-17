import re

with open('src/stores/appStore.ts', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('await api.invoke<UnifiedNode[]>', 'await invoke<UnifiedNode[]>')
content = content.replace('await api.invoke(', 'await invoke(')
content = content.replace("import { api } from '../services/api';", "import { api } from '../services/api';\nimport { invoke } from '@tauri-apps/api/core';")

with open('src/stores/appStore.ts', 'w', encoding='utf-8') as f:
    f.write(content)
