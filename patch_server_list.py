import re

with open('src/components/ServerList.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

# Add Rocket icon
content = content.replace('Gauge,\n  Sparkles,\n', 'Gauge,\n  Sparkles,\n  Rocket,\n')

# Add to destructuring
content = content.replace(
    '    setActiveTab,\n  } = useAppStore();',
    '    setActiveTab,\n    inspectProgress,\n    startDeepInspection,\n  } = useAppStore();'
)

# Insert Inspect Progress Modal
modal_code = '''
  const renderInspectModal = () => {
    if (!inspectProgress) return null;
    const percentage = inspectProgress.total > 0 ? (inspectProgress.current / inspectProgress.total) * 100 : 0;
    
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
        <div className="bg-surface p-6 rounded-xl shadow-2xl w-full max-w-md border border-white/10 relative overflow-hidden">
          <div className="absolute top-0 left-0 h-1 bg-primary/20 w-full">
            <div 
              className="h-full bg-primary transition-all duration-300"
              style={{ width: `${percentage}%` }}
            />
          </div>
          <div className="flex items-center space-x-4 mb-6 mt-2">
            <div className="w-12 h-12 rounded-full bg-primary/20 flex items-center justify-center animate-pulse">
              <Rocket className="w-6 h-6 text-primary" />
            </div>
            <div>
              <h3 className="text-lg font-bold text-white">深度节点质检中</h3>
              <p className="text-sm text-white/50">正在测真延迟、查真国家并清洗名称</p>
            </div>
          </div>
          
          <div className="space-y-4">
            <div className="flex justify-between text-sm text-white/70">
              <span>{inspectProgress.status}</span>
              <span className="font-mono">{inspectProgress.current} / {inspectProgress.total}</span>
            </div>
            <div className="h-2 bg-white/5 rounded-full overflow-hidden">
              <div 
                className="h-full bg-primary transition-all duration-300 rounded-full"
                style={{ width: `${percentage}%` }}
              />
            </div>
          </div>
        </div>
      </div>
    );
  };
'''

content = content.replace('return (', modal_code + '\n  return (', 1)

# Add button
button_code = '''
            <button
              onClick={() => startDeepInspection(filteredNodes)}
              disabled={!!inspectProgress || filteredNodes.length === 0}
              className="flex items-center space-x-2 px-3 py-1.5 rounded-lg bg-indigo-500/10 text-indigo-400 hover:bg-indigo-500/20 disabled:opacity-50 transition-colors"
            >
              <Rocket className="w-4 h-4" />
              <span className="text-sm font-medium">深度质检</span>
            </button>
'''

content = content.replace(
    '            <button\n              onClick={testAllSpeeds}',
    button_code + '            <button\n              onClick={testAllSpeeds}'
)

with open('src/components/ServerList.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
