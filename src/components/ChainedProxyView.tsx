import React, { useState, useMemo } from 'react';
import {
  Link2,
  Plus,
  Zap,
  Trash2,
  Edit2,
  ArrowRight,
  RefreshCw,
  Search,
  AlertCircle,
  X,
  ArrowUp,
  ArrowDown,
  Layers,
  Maximize2,
  Minimize2,
  Filter,
} from 'lucide-react';
import { useAppStore } from '../stores/appStore';
import { ProxyChain } from '../types';

export const ChainedProxyView: React.FC = () => {
  const {
    chains,
    nodes,
    subscriptions,
    connectedChainId,
    status,
    testingChainIds,
    connectChain,
    disconnect,
    deleteChain,
    addChain,
    updateChain,
    testChainLatency,
    testAllChains,
  } = useAppStore();

  const [searchQuery, setSearchQuery] = useState('');
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [isModalMaximized, setIsModalMaximized] = useState(false);
  const [editingChain, setEditingChain] = useState<ProxyChain | null>(null);

  // Form State
  const [formName, setFormName] = useState('');
  const [formRemarks, setFormRemarks] = useState('');
  const [formNodeIds, setFormNodeIds] = useState<string[]>([]);
  const [selectedGroupFilter, setSelectedGroupFilter] = useState<string>('all');
  const [selectedAddNodeId, setSelectedAddNodeId] = useState<string>('');
  const [modalError, setModalError] = useState<string | null>(null);

  // Delete Confirm State
  const [deletingChainId, setDeletingChainId] = useState<string | null>(null);

  // Calculate group node counts
  const groupCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const node of nodes) {
      const g = node.group?.trim() || 'Default';
      counts[g] = (counts[g] || 0) + 1;
    }
    return counts;
  }, [nodes]);

  // Extract unique groups from nodes and subscriptions
  const availableGroups = useMemo(() => {
    const groupSet = new Set<string>();
    for (const sub of subscriptions) {
      if (sub.name) groupSet.add(sub.name);
    }
    for (const node of nodes) {
      if (node.group && node.group.trim()) groupSet.add(node.group);
    }
    return ['all', ...Array.from(groupSet)];
  }, [subscriptions, nodes]);

  // Filtered nodes for the "Add Hop" selector based on selected group
  const filteredAddNodes = useMemo(() => {
    if (selectedGroupFilter === 'all') return nodes;
    return nodes.filter((n) => (n.group?.trim() || 'Default') === selectedGroupFilter);
  }, [nodes, selectedGroupFilter]);

  const handleGroupFilterChange = (group: string) => {
    setSelectedGroupFilter(group);
    const candidateNodes = group === 'all'
      ? nodes
      : nodes.filter((n) => (n.group?.trim() || 'Default') === group);
    setSelectedAddNodeId(candidateNodes[0]?.id || '');
  };

  // Filtered Chains
  const filteredChains = useMemo(() => {
    if (!searchQuery.trim()) return chains;
    const q = searchQuery.toLowerCase();
    return chains.filter((chain) => {
      if (chain.name.toLowerCase().includes(q)) return true;
      if (chain.remarks?.toLowerCase().includes(q)) return true;
      // Also match node names inside the chain
      return chain.node_ids.some((nid) => {
        const n = nodes.find((node) => node.id === nid);
        return n && (n.name.toLowerCase().includes(q) || n.address.toLowerCase().includes(q));
      });
    });
  }, [chains, searchQuery, nodes]);

  // Open Create Modal
  const handleOpenCreate = () => {
    setEditingChain(null);
    setFormName('');
    setFormRemarks('');
    setFormNodeIds([]);
    setSelectedGroupFilter('all');
    setSelectedAddNodeId(nodes[0]?.id || '');
    setModalError(null);
    setIsModalMaximized(false);
    setIsModalOpen(true);
  };

  // Open Edit Modal
  const handleOpenEdit = (chain: ProxyChain) => {
    setEditingChain(chain);
    setFormName(chain.name);
    setFormRemarks(chain.remarks || '');
    setFormNodeIds([...chain.node_ids]);
    setSelectedGroupFilter('all');
    setSelectedAddNodeId(nodes[0]?.id || '');
    setModalError(null);
    setIsModalMaximized(false);
    setIsModalOpen(true);
  };

  // Move Node Up in Chain
  const handleMoveUp = (index: number) => {
    if (index <= 0) return;
    setFormNodeIds((prev) => {
      const next = [...prev];
      const temp = next[index - 1];
      next[index - 1] = next[index];
      next[index] = temp;
      return next;
    });
  };

  // Move Node Down in Chain
  const handleMoveDown = (index: number) => {
    if (index >= formNodeIds.length - 1) return;
    setFormNodeIds((prev) => {
      const next = [...prev];
      const temp = next[index + 1];
      next[index + 1] = next[index];
      next[index] = temp;
      return next;
    });
  };

  // Remove Node from Chain
  const handleRemoveNode = (index: number) => {
    setFormNodeIds((prev) => prev.filter((_, i) => i !== index));
  };

  // Add Node to Chain
  const handleAddNodeToChain = () => {
    if (!selectedAddNodeId) return;
    setFormNodeIds((prev) => [...prev, selectedAddNodeId]);
  };

  // Save Modal
  const handleSaveChain = async () => {
    if (!formName.trim()) {
      setModalError('请输入链式代理名称 (Chain Name is required)');
      return;
    }
    if (formNodeIds.length < 2) {
      setModalError('链式代理至少需要选择 2 个跳板节点 (前置入口和最终落地出口)');
      return;
    }

    setModalError(null);
    if (editingChain) {
      const ok = await updateChain({
        ...editingChain,
        name: formName.trim(),
        remarks: formRemarks.trim(),
        node_ids: formNodeIds,
      });
      if (ok) setIsModalOpen(false);
    } else {
      const ok = await addChain({
        name: formName.trim(),
        remarks: formRemarks.trim(),
        node_ids: formNodeIds,
      });
      if (ok) setIsModalOpen(false);
    }
  };

  // Confirm Delete
  const handleConfirmDelete = async () => {
    if (!deletingChainId) return;
    await deleteChain(deletingChainId);
    setDeletingChainId(null);
  };

  return (
    <div className="flex-1 flex flex-col h-full bg-[#090a0f] p-6 overflow-hidden">
      {/* Top Header & Action Bar */}
      <div className="flex items-center justify-between gap-4 mb-5 shrink-0">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-blue-600/15 border border-blue-500/30 flex items-center justify-center text-blue-400 shadow-sm shadow-blue-500/10">
            <Link2 className="w-5 h-5" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <h1 className="text-base font-bold text-gray-100 tracking-tight">Chained Proxy · 链式代理</h1>
              <span className="px-2 py-0.5 rounded-md bg-[#161a26] text-blue-400 border border-blue-500/25 text-[11px] font-mono">
                {chains.length} 个配置
              </span>
            </div>
            <p className="text-xs text-gray-400 mt-0.5">
              参考 v2rayN 经典多级跳转机制：前置入口节点 ➔ 中继跃迁 ➔ 落地访问出口
            </p>
          </div>
        </div>

        {/* Action Controls */}
        <div className="flex items-center gap-2.5">
          {/* Search Bar */}
          <div className="relative w-56">
            <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
            <input
              type="text"
              placeholder="搜索链式代理..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full bg-[#12151f] border border-[#23293a] rounded-xl pl-9 pr-3 py-1.5 text-xs text-gray-200 placeholder-gray-500 focus:outline-none focus:border-blue-500/60 transition-colors"
            />
          </div>

          {/* Test All Chains */}
          <button
            onClick={() => testAllChains()}
            disabled={chains.length === 0}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[#141722] hover:bg-[#1c2233] text-gray-300 hover:text-white text-xs font-medium border border-[#23293a] transition-all disabled:opacity-40 shadow-sm"
            title="对所有链式代理测速"
          >
            <Zap className="w-3.5 h-3.5 text-amber-400" />
            <span>全部测速</span>
          </button>

          {/* Create Chain Button */}
          <button
            onClick={handleOpenCreate}
            className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl bg-blue-600 hover:bg-blue-500 text-white text-xs font-medium shadow-sm shadow-blue-500/20 transition-all"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>新建链式代理</span>
          </button>
        </div>
      </div>

      {/* Main Chains List Area */}
      <div className="flex-1 overflow-y-auto pr-1 space-y-3.5">
        {filteredChains.length === 0 ? (
          <div className="h-64 flex flex-col items-center justify-center rounded-2xl border border-[#1a1e2b] bg-[#0e1017]/60 p-8 text-center">
            <div className="w-12 h-12 rounded-2xl bg-blue-500/10 border border-blue-500/20 flex items-center justify-center text-blue-400 mb-3">
              <Layers className="w-6 h-6" />
            </div>
            <h3 className="text-sm font-semibold text-gray-200 mb-1">
              {chains.length === 0 ? '暂无自定义链式代理' : '未找到匹配的链式代理'}
            </h3>
            <p className="text-xs text-gray-500 max-w-sm mb-4 leading-relaxed">
              {chains.length === 0
                ? '链式代理允许将多个节点按次序串联，流量由前置跳板跳转至落地出口，突破复杂隔离并隐藏真实网络身份。'
                : '请尝试更换搜索关键字。'}
            </p>
            {chains.length === 0 && (
              <button
                onClick={handleOpenCreate}
                className="flex items-center gap-1.5 px-4 py-2 rounded-xl bg-blue-600 hover:bg-blue-500 text-white text-xs font-medium shadow-md shadow-blue-500/20 transition-all"
              >
                <Plus className="w-4 h-4" />
                <span>创建第一个链式代理</span>
              </button>
            )}
          </div>
        ) : (
          filteredChains.map((chain) => {
            const isConnected = connectedChainId === chain.id && status === 'connected';
            const isConnecting = connectedChainId === chain.id && status === 'connecting';
            const isTesting = testingChainIds.includes(chain.id);

            // Resolve chain node objects
            const resolvedHops = chain.node_ids.map((nid, idx) => {
              const node = nodes.find((n) => n.id === nid);
              return { nid, idx, node };
            });

            return (
              <div
                key={chain.id}
                className={`w-full rounded-2xl border transition-all p-4 ${
                  isConnected
                    ? 'bg-emerald-950/15 border-emerald-500/40 shadow-lg shadow-emerald-500/5'
                    : isConnecting
                    ? 'bg-blue-950/15 border-blue-500/40 shadow-lg shadow-blue-500/5'
                    : 'bg-[#10131d] border-[#1d2232] hover:border-[#2b334a]'
                }`}
              >
                {/* Card Top Row: Name, Badges & Actions */}
                <div className="flex items-center justify-between gap-3 mb-3.5">
                  <div className="flex items-center gap-3">
                    <div
                      className={`w-8 h-8 rounded-xl flex items-center justify-center font-bold text-xs shrink-0 ${
                        isConnected
                          ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/30'
                          : 'bg-[#181c2b] text-gray-300 border border-[#272e44]'
                      }`}
                    >
                      <Link2 className="w-4 h-4" />
                    </div>

                    <div>
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-bold text-gray-100">{chain.name}</span>
                        <span className="px-2 py-0.5 rounded-full bg-[#181d2c] text-indigo-300 border border-indigo-500/30 text-[10px] font-mono font-medium">
                          {chain.node_ids.length} 级跳转
                        </span>
                        {/* Live State Badge */}
                        {isConnected ? (
                          <span className="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-emerald-500/15 text-emerald-300 border border-emerald-500/30 text-[10px] font-medium">
                            <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                            已连接
                          </span>
                        ) : isConnecting ? (
                          <span className="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-blue-500/15 text-blue-300 border border-blue-500/30 text-[10px] font-medium">
                            <RefreshCw className="w-2.5 h-2.5 animate-spin" />
                            连接中
                          </span>
                        ) : null}
                      </div>
                      {chain.remarks && (
                        <p className="text-xs text-gray-400 mt-0.5 font-sans">{chain.remarks}</p>
                      )}
                    </div>
                  </div>

                  {/* Right side: Latency badge + Action Buttons */}
                  <div className="flex items-center gap-2">
                    {/* Latency badge */}
                    <div
                      onClick={() => testChainLatency(chain.id)}
                      className="cursor-pointer flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-[#161a26] hover:bg-[#1e2436] border border-[#252b3d] text-xs transition-colors"
                      title="点击对本链测速"
                    >
                      {isTesting ? (
                        <>
                          <RefreshCw className="w-3 h-3 animate-spin text-amber-400" />
                          <span className="text-amber-400 font-mono text-[11px]">测速中...</span>
                        </>
                      ) : chain.latency_ms !== undefined && chain.latency_ms !== null ? (
                        chain.latency_ms < 0 ? (
                          <span className="text-red-400 font-mono text-[11px] font-semibold">超时</span>
                        ) : (
                          <span
                            className={`font-mono text-[11px] font-semibold ${
                              chain.latency_ms < 150
                                ? 'text-emerald-400'
                                : chain.latency_ms < 300
                                ? 'text-amber-400'
                                : 'text-red-400'
                            }`}
                          >
                            {chain.latency_ms} ms
                          </span>
                        )
                      ) : (
                        <span className="text-gray-500 font-mono text-[11px]">未测速</span>
                      )}
                    </div>

                    {/* Test button */}
                    <button
                      onClick={() => testChainLatency(chain.id)}
                      disabled={isTesting}
                      className="p-1.5 rounded-lg bg-[#161a26] hover:bg-[#1e2436] text-gray-300 hover:text-amber-300 border border-[#252b3d] transition-colors disabled:opacity-40"
                      title="测试链路延迟"
                    >
                      <Zap className={`w-3.5 h-3.5 ${isTesting ? 'animate-spin text-amber-400' : ''}`} />
                    </button>

                    {/* Edit button */}
                    <button
                      onClick={() => handleOpenEdit(chain)}
                      className="p-1.5 rounded-lg bg-[#161a26] hover:bg-[#1e2436] text-gray-300 hover:text-white border border-[#252b3d] transition-colors"
                      title="编辑代理链"
                    >
                      <Edit2 className="w-3.5 h-3.5" />
                    </button>

                    {/* Delete button */}
                    <button
                      onClick={() => setDeletingChainId(chain.id)}
                      className="p-1.5 rounded-lg bg-[#161a26] hover:bg-red-500/20 text-gray-400 hover:text-red-400 border border-[#252b3d] hover:border-red-500/30 transition-colors"
                      title="删除代理链"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>

                    {/* Connect / Disconnect button */}
                    {isConnected ? (
                      <button
                        onClick={() => disconnect()}
                        className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl bg-amber-600/20 hover:bg-amber-600/30 text-amber-300 border border-amber-500/40 text-xs font-semibold transition-all shadow-sm"
                      >
                        <span>断开连接</span>
                      </button>
                    ) : (
                      <button
                        onClick={() => connectChain(chain.id)}
                        disabled={isConnecting}
                        className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl bg-blue-600 hover:bg-blue-500 text-white text-xs font-semibold shadow-sm shadow-blue-500/25 transition-all disabled:opacity-50"
                      >
                        {isConnecting ? (
                          <>
                            <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                            <span>连接中...</span>
                          </>
                        ) : (
                          <span>一键连接</span>
                        )}
                      </button>
                    )}
                  </div>
                </div>

                {/* Topology Flow Visualization */}
                <div className="bg-[#0b0d14] rounded-xl p-3 border border-[#191d2a] overflow-x-auto">
                  <div className="flex items-center gap-2 min-w-max">
                    {resolvedHops.map(({ nid, node }, i) => {
                      const isFirst = i === 0;
                      const isLast = i === resolvedHops.length - 1;

                      return (
                        <React.Fragment key={`${nid}-${i}`}>
                          {/* Hop Node Box */}
                          <div
                            className={`flex items-center gap-2.5 px-3 py-2 rounded-xl border transition-all ${
                              isLast
                                ? 'bg-indigo-950/20 border-indigo-500/40 shadow-sm'
                                : isFirst
                                ? 'bg-blue-950/20 border-blue-500/40'
                                : 'bg-[#12151f] border-[#22283a]'
                            }`}
                          >
                            {/* Step Badge */}
                            <div className="flex flex-col items-start">
                              <span
                                className={`text-[10px] font-bold uppercase tracking-wider px-1.5 py-0.2 rounded ${
                                  isFirst
                                    ? 'bg-blue-500/20 text-blue-300'
                                    : isLast
                                    ? 'bg-purple-500/20 text-purple-300'
                                    : 'bg-gray-800 text-gray-400'
                                }`}
                              >
                                {isFirst ? '#1 前置' : isLast ? `#${i + 1} 落地` : `#${i + 1} 中继`}
                              </span>
                            </div>

                            {/* Node Info */}
                            {node ? (
                              <div className="flex items-center gap-2">
                                <span className="font-mono text-[11px] font-bold px-1.5 py-0.5 rounded bg-[#1c2130] text-gray-200 border border-[#2b334a]">
                                  {node.country_code || 'UN'}
                                </span>
                                <div className="max-w-[140px] truncate">
                                  <div className="text-xs font-medium text-gray-200 truncate" title={node.name}>
                                    {node.name}
                                  </div>
                                  <div className="text-[10px] text-gray-500 font-mono truncate">
                                    {node.protocol.toUpperCase()} · {node.address}:{node.port}
                                  </div>
                                </div>
                              </div>
                            ) : (
                              <div className="flex items-center gap-1.5 text-red-400 text-xs">
                                <AlertCircle className="w-3.5 h-3.5" />
                                <span className="text-[11px]">节点不存在 ({nid.slice(0, 8)})</span>
                              </div>
                            )}
                          </div>

                          {/* Arrow between hops */}
                          {!isLast && (
                            <div className="flex items-center justify-center text-blue-500/60 px-0.5 shrink-0">
                              <ArrowRight className="w-4 h-4" />
                            </div>
                          )}
                        </React.Fragment>
                      );
                    })}
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>

      {/* New / Edit Chain Modal Dialog */}
      {isModalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 animate-in fade-in duration-150">
          <div
            className={`bg-[#11141e] border border-[#22283a] rounded-2xl flex flex-col shadow-2xl relative transition-all duration-150 ${
              isModalMaximized
                ? 'w-[96vw] h-[94vh] max-w-[96vw] max-h-[94vh]'
                : 'w-[720px] h-[660px] min-w-[500px] min-h-[440px] max-w-[95vw] max-h-[92vh]'
            }`}
            style={isModalMaximized ? undefined : { resize: 'both', overflow: 'hidden' }}
          >
            {/* Modal Header */}
            <div className="px-5 py-3.5 border-b border-[#1f2436] flex items-center justify-between shrink-0 bg-[#0e111a]">
              <div className="flex items-center gap-2">
                <Link2 className="w-4 h-4 text-blue-400" />
                <h2 className="text-sm font-bold text-gray-100">
                  {editingChain ? '编辑链式代理 (Edit Chain)' : '新建链式代理 (New Proxy Chain)'}
                </h2>
              </div>
              <div className="flex items-center gap-1.5">
                <button
                  onClick={() => setIsModalMaximized(!isModalMaximized)}
                  className="text-gray-400 hover:text-white p-1.5 rounded-lg hover:bg-gray-800 transition-colors"
                  title={isModalMaximized ? '还原窗口' : '最大化窗口'}
                >
                  {isModalMaximized ? <Minimize2 className="w-4 h-4" /> : <Maximize2 className="w-4 h-4" />}
                </button>
                <button
                  onClick={() => setIsModalOpen(false)}
                  className="text-gray-400 hover:text-white p-1.5 rounded-lg hover:bg-gray-800 transition-colors"
                  title="关闭"
                >
                  <X className="w-4 h-4" />
                </button>
              </div>
            </div>

            {/* Modal Body */}
            <div className="p-5 flex-1 overflow-y-auto space-y-4">
              {modalError && (
                <div className="p-3 rounded-xl bg-red-950/60 border border-red-500/40 text-red-200 text-xs flex items-center gap-2">
                  <AlertCircle className="w-4 h-4 shrink-0 text-red-400" />
                  <span>{modalError}</span>
                </div>
              )}

              {/* Chain Name */}
              <div>
                <label className="block text-xs font-semibold text-gray-300 mb-1.5">
                  链名称 / 备注名 <span className="text-red-400">*</span>
                </label>
                <input
                  type="text"
                  placeholder="例如: HK前置 + JP落地 双跳专线"
                  value={formName}
                  onChange={(e) => setFormName(e.target.value)}
                  className="w-full bg-[#161a26] border border-[#262d40] rounded-xl px-3.5 py-2 text-xs text-gray-100 placeholder-gray-500 focus:outline-none focus:border-blue-500 transition-colors"
                />
              </div>

              {/* Remarks */}
              <div>
                <label className="block text-xs font-semibold text-gray-300 mb-1.5">
                  备注说明 (可选)
                </label>
                <input
                  type="text"
                  placeholder="例如: 经香港中继绕过封锁，日本节点访问流媒体"
                  value={formRemarks}
                  onChange={(e) => setFormRemarks(e.target.value)}
                  className="w-full bg-[#161a26] border border-[#262d40] rounded-xl px-3.5 py-2 text-xs text-gray-100 placeholder-gray-500 focus:outline-none focus:border-blue-500 transition-colors"
                />
              </div>

              {/* Hop Sequence Header */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <label className="text-xs font-semibold text-gray-300 flex items-center gap-1.5">
                    <span>跳转节点序列 (最少 2 级跳转)</span>
                    <span className="text-gray-500 font-normal">({formNodeIds.length} 已选)</span>
                  </label>
                </div>

                {/* Hops List */}
                {formNodeIds.length === 0 ? (
                  <div className="p-4 rounded-xl border border-dashed border-[#23293b] text-center text-xs text-gray-500">
                    请在下方选择节点并点击“添加至跳板序列”
                  </div>
                ) : (
                  <div className="space-y-2 max-h-56 overflow-y-auto pr-1">
                    {formNodeIds.map((nid, idx) => {
                      const node = nodes.find((n) => n.id === nid);
                      const isFirst = idx === 0;
                      const isLast = idx === formNodeIds.length - 1;

                      return (
                        <div
                          key={`${nid}-${idx}`}
                          className="flex items-center justify-between gap-2 p-2.5 rounded-xl bg-[#161a26] border border-[#242b3d]"
                        >
                          <div className="flex items-center gap-2.5 min-w-0">
                            <span
                              className={`text-[10px] font-bold px-2 py-0.5 rounded font-mono shrink-0 ${
                                isFirst
                                  ? 'bg-blue-500/20 text-blue-300 border border-blue-500/30'
                                  : isLast
                                  ? 'bg-purple-500/20 text-purple-300 border border-purple-500/30'
                                  : 'bg-gray-800 text-gray-400 border border-gray-700'
                              }`}
                            >
                              {isFirst ? '#1 前置' : isLast ? `#${idx + 1} 落地` : `#${idx + 1} 中继`}
                            </span>

                            {node ? (
                              <div className="min-w-0">
                                <div className="text-xs font-medium text-gray-200 truncate">{node.name}</div>
                                <div className="text-[10px] text-gray-500 font-mono">
                                  {node.protocol.toUpperCase()} · {node.country_code} · {node.address}:{node.port}
                                </div>
                              </div>
                            ) : (
                              <span className="text-xs text-red-400">未知节点 ({nid.slice(0, 8)})</span>
                            )}
                          </div>

                          {/* Order Controls & Delete */}
                          <div className="flex items-center gap-1 shrink-0">
                            <button
                              type="button"
                              onClick={() => handleMoveUp(idx)}
                              disabled={isFirst}
                              className="p-1 rounded-lg bg-[#1e2333] hover:bg-[#283046] text-gray-300 hover:text-white disabled:opacity-20 transition-colors"
                              title="上移此跳"
                            >
                              <ArrowUp className="w-3.5 h-3.5" />
                            </button>
                            <button
                              type="button"
                              onClick={() => handleMoveDown(idx)}
                              disabled={isLast}
                              className="p-1 rounded-lg bg-[#1e2333] hover:bg-[#283046] text-gray-300 hover:text-white disabled:opacity-20 transition-colors"
                              title="下移此跳"
                            >
                              <ArrowDown className="w-3.5 h-3.5" />
                            </button>
                            <button
                              type="button"
                              onClick={() => handleRemoveNode(idx)}
                              className="p-1 rounded-lg bg-[#1e2333] hover:bg-red-500/20 text-gray-400 hover:text-red-400 transition-colors ml-1"
                              title="移除此跳"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>

              {/* Add Node from Dropdown with Group Filter */}
              <div className="pt-3 border-t border-[#1e2335]">
                <div className="flex items-center justify-between mb-2">
                  <label className="text-xs font-semibold text-gray-300 flex items-center gap-1.5">
                    <Filter className="w-3.5 h-3.5 text-blue-400" />
                    <span>添加跳板节点 (按分组挑选节点)</span>
                  </label>
                  <span className="text-[11px] text-gray-500 font-mono">
                    当前分组: {filteredAddNodes.length} 个节点
                  </span>
                </div>

                <div className="grid grid-cols-1 md:grid-cols-12 gap-2.5">
                  {/* Step 1: Select Group */}
                  <div className="md:col-span-4">
                    <label className="block text-[11px] text-gray-400 mb-1">
                      1. 选择节点分组:
                    </label>
                    <select
                      value={selectedGroupFilter}
                      onChange={(e) => handleGroupFilterChange(e.target.value)}
                      className="w-full bg-[#161a26] border border-[#262d40] rounded-xl px-3 py-2 text-xs text-gray-200 focus:outline-none focus:border-blue-500 transition-colors cursor-pointer"
                    >
                      <option value="all">全部分组 ({nodes.length})</option>
                      {availableGroups.filter((g) => g !== 'all').map((g) => (
                        <option key={g} value={g}>
                          {g} ({groupCounts[g] || 0})
                        </option>
                      ))}
                    </select>
                  </div>

                  {/* Step 2: Select Node */}
                  <div className="md:col-span-6">
                    <label className="block text-[11px] text-gray-400 mb-1">
                      2. 选择具体节点:
                    </label>
                    <select
                      value={selectedAddNodeId}
                      onChange={(e) => setSelectedAddNodeId(e.target.value)}
                      className="w-full bg-[#161a26] border border-[#262d40] rounded-xl px-3 py-2 text-xs text-gray-200 focus:outline-none focus:border-blue-500 transition-colors cursor-pointer"
                    >
                      {filteredAddNodes.length === 0 ? (
                        <option value="">(该分组暂无可添加节点)</option>
                      ) : (
                        filteredAddNodes.map((node) => (
                          <option key={node.id} value={node.id}>
                            [{node.country_code || 'UN'}] [{node.protocol.toUpperCase()}] {node.name} ({node.address})
                          </option>
                        ))
                      )}
                    </select>
                  </div>

                  {/* Step 3: Add button */}
                  <div className="md:col-span-2 flex items-end">
                    <button
                      type="button"
                      onClick={handleAddNodeToChain}
                      disabled={!selectedAddNodeId}
                      className="w-full flex items-center justify-center gap-1.5 px-3 py-2 rounded-xl bg-[#1e2333] hover:bg-[#283046] text-blue-400 border border-blue-500/30 text-xs font-medium transition-colors shrink-0 disabled:opacity-40 shadow-sm"
                      title="添加所选节点到跳板序列"
                    >
                      <Plus className="w-3.5 h-3.5" />
                      <span>添加</span>
                    </button>
                  </div>
                </div>
              </div>

              {/* Informational guide */}
              <div className="p-3 rounded-xl bg-blue-950/20 border border-blue-500/20 text-[11px] text-gray-400 leading-relaxed">
                💡 <span className="text-gray-300 font-medium">链路转发原理：</span>
                数据将严格按照「前置节点 ➔ 中继节点 ➔ 落地出口」的次序逐级包装发送（Sing-box 原生多级 detour 机制）。
              </div>
            </div>

            {/* Modal Footer */}
            <div className="px-5 py-3.5 border-t border-[#1f2436] bg-[#0e111a] flex items-center justify-between shrink-0 relative">
              <span className="text-[11px] text-gray-500 font-mono select-none">
                💡 可拖拽右下角或点击右上角调整窗口大小
              </span>
              <div className="flex items-center gap-2.5">
                <button
                  type="button"
                  onClick={() => setIsModalOpen(false)}
                  className="px-4 py-2 rounded-xl bg-[#161a26] hover:bg-[#1e2333] text-gray-300 text-xs font-medium border border-[#252b3d] transition-colors"
                >
                  取消
                </button>
                <button
                  type="button"
                  onClick={handleSaveChain}
                  className="px-4 py-2 rounded-xl bg-blue-600 hover:bg-blue-500 text-white text-xs font-semibold shadow-sm shadow-blue-500/25 transition-all"
                >
                  保存链式代理
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Delete Confirmation Modal */}
      {deletingChainId && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 animate-in fade-in duration-150">
          <div className="bg-[#11141e] border border-[#242b3d] rounded-2xl w-full max-w-sm p-5 shadow-2xl">
            <div className="flex items-center gap-3 mb-3 text-red-400">
              <AlertCircle className="w-5 h-5 shrink-0" />
              <h3 className="text-sm font-bold text-gray-100">确认删除链式代理？</h3>
            </div>
            <p className="text-xs text-gray-400 mb-5 leading-relaxed">
              确定要删除该链式代理配置吗？此操作无法撤销。
            </p>
            <div className="flex items-center justify-end gap-2.5">
              <button
                onClick={() => setDeletingChainId(null)}
                className="px-3.5 py-1.5 rounded-xl bg-[#161a26] hover:bg-[#1e2333] text-gray-300 text-xs font-medium border border-[#252b3d] transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleConfirmDelete}
                className="px-3.5 py-1.5 rounded-xl bg-red-600 hover:bg-red-500 text-white text-xs font-semibold shadow-sm shadow-red-500/20 transition-all"
              >
                确认删除
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
