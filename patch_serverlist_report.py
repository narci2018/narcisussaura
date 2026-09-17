import re

with open('src/components/ServerList.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

# Make sure X is imported for the close button
if 'X, ' not in content and '{ X' not in content:
    content = content.replace('Trash2,', 'Trash2, X, CheckCircle,')

# Add missing state imports
content = content.replace('inspectProgress,', 'inspectProgress,\n    inspectReport,\n    closeInspectReport,')

# Add renderInspectReport
report_component = '''
  const renderInspectReport = () => {
    if (!inspectReport) return null;
    
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
        <div className="bg-[#12151f] p-6 rounded-2xl shadow-2xl w-full max-w-2xl border border-[#212637] flex flex-col max-h-[85vh]">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-full bg-emerald-500/20 flex items-center justify-center text-emerald-400">
                <CheckCircle className="w-5 h-5" />
              </div>
              <div>
                <h3 className="text-xl font-bold text-gray-100">深度质检报告</h3>
                <p className="text-sm text-gray-500">检测并修复了 {inspectReport.items.length} 个异常节点</p>
              </div>
            </div>
            <button 
              onClick={closeInspectReport}
              className="w-8 h-8 flex items-center justify-center rounded-full hover:bg-white/10 text-gray-400 transition-colors"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
          
          <div className="flex-1 overflow-y-auto pr-2 space-y-3 custom-scrollbar">
            {inspectReport.items.length === 0 ? (
              <div className="text-center text-gray-500 py-10">所有节点信息准确，无需修复。</div>
            ) : (
              inspectReport.items.map(item => (
                <div key={item.id} className="bg-[#181c28] border border-[#212637] rounded-xl p-4 flex flex-col gap-2">
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-semibold text-gray-300 truncate mr-2" title={item.oldName}>
                      {item.oldName}
                    </span>
                    <span className="text-xs px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-400 border border-blue-500/20 shrink-0">
                      修复后 ➔
                    </span>
                  </div>
                  <div className="text-sm font-semibold text-emerald-400 truncate" title={item.newName}>
                    {item.newName}
                  </div>
                  <div className="grid grid-cols-3 gap-2 mt-2 pt-2 border-t border-white/5 text-xs">
                    <div>
                      <div className="text-gray-500 mb-0.5">国家地区</div>
                      <div className="flex items-center gap-1">
                        <span className="text-red-400 line-through">{item.oldCountry}</span>
                        <span className="text-gray-600">→</span>
                        <span className="text-emerald-400 font-bold">{item.newCountry}</span>
                      </div>
                    </div>
                    <div>
                      <div className="text-gray-500 mb-0.5">真实延迟</div>
                      <div className="flex items-center gap-1">
                        <span className={item.oldLatency ? 'text-red-400 line-through' : 'text-gray-500'}>
                          {item.oldLatency ? ${item.oldLatency}ms : '无'}
                        </span>
                        <span className="text-gray-600">→</span>
                        <span className="text-emerald-400 font-mono">{item.newLatency}ms</span>
                      </div>
                    </div>
                    <div>
                      <div className="text-gray-500 mb-0.5">下行带宽</div>
                      <div className="flex items-center gap-1">
                        <span className={item.oldSpeed ? 'text-red-400 line-through' : 'text-gray-500'}>
                          {item.oldSpeed ? ${(item.oldSpeed / (1024 * 1024)).toFixed(1)}M : '无'}
                        </span>
                        <span className="text-gray-600">→</span>
                        <span className="text-emerald-400 font-mono">
                          {item.newSpeed ? ${(item.newSpeed / (1024 * 1024)).toFixed(1)}M : '0M'}
                        </span>
                      </div>
                    </div>
                  </div>
                </div>
              ))
            )}
          </div>
          
          <div className="mt-6 flex justify-end">
            <button
              onClick={closeInspectReport}
              className="px-6 py-2 bg-indigo-600 hover:bg-indigo-500 text-white font-medium rounded-xl transition-colors shadow-lg shadow-indigo-600/20"
            >
              完成并关闭
            </button>
          </div>
        </div>
      </div>
    );
  };
'''
content = content.replace('  const renderInspectModal = () => {', report_component + '\n  const renderInspectModal = () => {')

content = content.replace('{renderInspectModal()}', '{renderInspectModal()}\n      {renderInspectReport()}')

with open('src/components/ServerList.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
