import re

with open('src/components/ServerList.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

# Replace any malformed line containing oldLatency? $
content = re.sub(r'\{item\.oldLatency \? \$\{item\.oldLatency\}ms : [^\}]+\}', r"{item.oldLatency ? ${item.oldLatency}ms : '无'}", content)
content = re.sub(r'\{item\.oldSpeed \? \$\{\(item\.oldSpeed / \(1024 \* 1024\)\)\.toFixed\(1\)\}M : [^\}]+\}', r"{item.oldSpeed ? ${(item.oldSpeed / (1024 * 1024)).toFixed(1)}M : '无'}", content)
content = re.sub(r'\{item\.newSpeed \? \$\{\(item\.newSpeed / \(1024 \* 1024\)\)\.toFixed\(1\)\}M : [^\}]+\}', r"{item.newSpeed ? ${(item.newSpeed / (1024 * 1024)).toFixed(1)}M : '0M'}", content)

with open('src/components/ServerList.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
