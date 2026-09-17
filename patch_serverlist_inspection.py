import re

with open('src/components/ServerList.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('onClick={() => startDeepInspection(nodes)}', 'onClick={() => startDeepInspection(filteredAndSortedNodes)}')
content = content.replace('disabled={!!inspectProgress || nodes.length === 0}', 'disabled={!!inspectProgress || filteredAndSortedNodes.length === 0}')

with open('src/components/ServerList.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
