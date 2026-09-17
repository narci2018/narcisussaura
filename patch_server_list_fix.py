import re

with open('src/components/ServerList.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

# Fix unused modal (insert it before return ( )
modal_usage = '''
  return (
    <div className="h-full flex flex-col relative">
      {renderInspectModal()}
'''
content = content.replace('  return (\n    <div className="h-full flex flex-col relative">', modal_usage)


button_code = '''
          <button
            onClick={() => startDeepInspection(filteredNodes)}
            disabled={!!inspectProgress || filteredNodes.length === 0}
            className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl bg-purple-500/10 hover:bg-purple-500/20 text-purple-400 hover:text-purple-300 text-xs font-medium border border-purple-500/30 transition-all disabled:opacity-50 shadow-sm"
          >
            <Rocket className="w-3.5 h-3.5" />
            <span>深度质检</span>
          </button>
'''
content = content.replace(
    '          {/* Test All Speed */}\n          <button\n            onClick={() => testAllSpeeds()}',
    button_code + '          {/* Test All Speed */}\n          <button\n            onClick={() => testAllSpeeds()}'
)

with open('src/components/ServerList.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
