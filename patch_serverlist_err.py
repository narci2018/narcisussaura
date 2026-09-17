import re

with open('src/components/ServerList.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace("{item.oldLatency ? ms : '无'}", "{item.oldLatency ? ${item.oldLatency}ms : '无'}")
content = content.replace("{item.oldSpeed ? M : '无'}", "{item.oldSpeed ? ${(item.oldSpeed / (1024 * 1024)).toFixed(1)}M : '无'}")
content = content.replace("{item.newSpeed ? M : '0M'}", "{item.newSpeed ? ${(item.newSpeed / (1024 * 1024)).toFixed(1)}M : '0M'}")

with open('src/components/ServerList.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
