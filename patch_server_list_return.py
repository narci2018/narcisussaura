import re

with open('src/components/ServerList.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace(
'''  return (
    <div className="flex-1 flex flex-col overflow-hidden px-6 py-4 max-w-[1600px] mx-auto w-full">''',
'''  return (
    <div className="flex-1 flex flex-col overflow-hidden px-6 py-4 max-w-[1600px] mx-auto w-full relative">
      {renderInspectModal()}'''
)

with open('src/components/ServerList.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
