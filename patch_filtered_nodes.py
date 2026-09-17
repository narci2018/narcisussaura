import re

with open('src/components/ServerList.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('filteredNodes', 'nodes')
content = content.replace(
'''  const renderInspectModal = () => {''',
'''
  const renderInspectModal = () => {'''
)

# wait, the modal was unused because I probably failed to insert {renderInspectModal()}
# Let's check where it should be
import sys
if '{renderInspectModal()}' not in content:
    content = content.replace(
        '  return (\n    <div className="h-full flex flex-col relative">',
        '  return (\n    <div className="h-full flex flex-col relative">\n      {renderInspectModal()}'
    )

with open('src/components/ServerList.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
