import re

with open('src/components/ServerList.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

content = re.sub(r'\{item\.oldLatency \? \$\{item\.oldLatency\}ms : \'无\'\}', r"{item.oldLatency ? ${item.oldLatency}ms : '无'}", content)
content = re.sub(r'\{item\.oldSpeed \? \$\{\(item\.oldSpeed / \(1024 \* 1024\)\)\.toFixed\(1\)\}M : \'无\'\}', r"{item.oldSpeed ? ${(item.oldSpeed / (1024 * 1024)).toFixed(1)}M : '无'}", content)
content = re.sub(r'\{item\.newSpeed \? \$\{\(item\.newSpeed / \(1024 \* 1024\)\)\.toFixed\(1\)\}M : \'0M\'\}', r"{item.newSpeed ? ${(item.newSpeed / (1024 * 1024)).toFixed(1)}M : '0M'}", content)

with open('src/components/ServerList.tsx', 'w', encoding='utf-8') as f:
    f.write(content)

with open('src/stores/appStore.ts', 'r', encoding='utf-8') as f:
    content2 = f.read()
content2 = re.sub(r'set\(\{ errorMessage: Deep inspection failed:  \}\);', r"set({ errorMessage: Deep inspection failed:  });", content2)
with open('src/stores/appStore.ts', 'w', encoding='utf-8') as f:
    f.write(content2)
