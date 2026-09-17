import re

with open('src-tauri/Cargo.toml', 'r', encoding='utf-8') as f:
    content = f.read()

content = re.sub(r'version = "0\.2\.19"', 'version = "0.2.20"', content, count=1)

with open('src-tauri/Cargo.toml', 'w', encoding='utf-8') as f:
    f.write(content)
