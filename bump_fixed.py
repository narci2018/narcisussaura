import json
import sys

version = sys.argv[1] if len(sys.argv) > 1 else '0.2.22'

with open('package.json', 'r', encoding='utf-8') as f:
    pkg = json.load(f)
pkg['version'] = version
with open('package.json', 'w', encoding='utf-8') as f:
    json.dump(pkg, f, indent=2)

with open('src-tauri/tauri.conf.json', 'r', encoding='utf-8') as f:
    conf = json.load(f)
conf['version'] = version
with open('src-tauri/tauri.conf.json', 'w', encoding='utf-8') as f:
    json.dump(conf, f, indent=2)

print(f"Bumped JSON files to {version}")
