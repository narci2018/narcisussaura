import json

with open('package.json', 'r', encoding='utf-8') as f:
    pkg = json.load(f)
pkg['version'] = '0.2.20'
with open('package.json', 'w', encoding='utf-8') as f:
    json.dump(pkg, f, indent=2)

with open('src-tauri/tauri.conf.json', 'r', encoding='utf-8') as f:
    conf = json.load(f)
conf['version'] = '0.2.20'
with open('src-tauri/tauri.conf.json', 'w', encoding='utf-8') as f:
    json.dump(conf, f, indent=2)
