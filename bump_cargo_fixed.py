import sys
import re

version = sys.argv[1] if len(sys.argv) > 1 else '0.2.22'

with open('src-tauri/Cargo.toml', 'r', encoding='utf-8') as f:
    content = f.read()

content = re.sub(r'^version = ".*"', f'version = "{version}"', content, count=1, flags=re.MULTILINE)

with open('src-tauri/Cargo.toml', 'w', encoding='utf-8') as f:
    f.write(content)

print(f"Bumped Cargo.toml to {version}")
