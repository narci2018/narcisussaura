import re

with open('src-tauri/src/core/singbox.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Replace hardcoded "outbound": "proxy" with "outbound": dns_remote_detour in generate_config_common
content = content.replace('"outbound": "proxy"', '"outbound": dns_remote_detour')

with open('src-tauri/src/core/singbox.rs', 'w', encoding='utf-8') as f:
    f.write(content)
