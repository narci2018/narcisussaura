import React, { useState } from 'react';
import { Shield, Network, Cpu, Save, Check, Layers, Plus, Edit3, Trash2, X, RotateCcw } from 'lucide-react';
import { useAppStore } from '../stores/appStore';
import { AppSettings, ProxyMode, RoutingRuleSet } from '../types';

const DEFAULT_RECOMMENDED_RULE_SET: RoutingRuleSet = {
  id: 'default',
  name: '默认',
  direct_rules: ['geosite:cn', 'geoip:cn', 'geosite:private', 'geoip:private'],
  proxy_rules: [],
  block_rules: ['geosite:category-ads-all'],
};

export const SettingsView: React.FC = () => {
  const { settings, saveSettings } = useAppStore();
  const [formData, setFormData] = useState<AppSettings>({ ...settings });
  const [saved, setSaved] = useState(false);

  const ruleSets: RoutingRuleSet[] =
    formData.rule_sets && formData.rule_sets.length > 0
      ? formData.rule_sets
      : [DEFAULT_RECOMMENDED_RULE_SET];

  const activeRuleSet =
    ruleSets.find((s) => s.id === formData.active_rule_set_id) || ruleSets[0] || DEFAULT_RECOMMENDED_RULE_SET;

  const [modalOpen, setModalOpen] = useState(false);
  const [editingSet, setEditingSet] = useState<RoutingRuleSet>({ ...DEFAULT_RECOMMENDED_RULE_SET });
  const [isNewSet, setIsNewSet] = useState(false);

  const handleOpenEdit = (ruleSetToEdit?: RoutingRuleSet) => {
    const target = ruleSetToEdit || activeRuleSet;
    setEditingSet({
      id: target.id,
      name: target.name,
      direct_rules: [...(target.direct_rules || [])],
      proxy_rules: [...(target.proxy_rules || [])],
      block_rules: [...(target.block_rules || [])],
    });
    setIsNewSet(false);
    setModalOpen(true);
  };

  const handleOpenCreateNew = () => {
    setEditingSet({
      id: 'ruleset_' + Date.now(),
      name: '新建规则集',
      direct_rules: ['geosite:cn', 'geoip:cn', 'geosite:private', 'geoip:private'],
      proxy_rules: [],
      block_rules: ['geosite:category-ads-all'],
    });
    setIsNewSet(true);
    setModalOpen(true);
  };

  const handleSelectRuleSet = async (newId: string) => {
    const target = ruleSets.find((s) => s.id === newId) || ruleSets[0];
    const updatedFormData: AppSettings = {
      ...formData,
      active_rule_set_id: newId,
      direct_geo_rules: target.direct_rules.filter((r) => r.startsWith('geosite:') || r.startsWith('geoip:')),
      block_geo_rules: target.block_rules.filter((r) => r.startsWith('geosite:') || r.startsWith('geoip:')),
      proxy_geo_rules: target.proxy_rules.filter((r) => r.startsWith('geosite:') || r.startsWith('geoip:')),
    };
    setFormData(updatedFormData);
    await saveSettings(updatedFormData);
  };

  const handleSaveRuleSet = async () => {
    const curSets = formData.rule_sets && formData.rule_sets.length > 0
      ? [...formData.rule_sets]
      : [{ ...DEFAULT_RECOMMENDED_RULE_SET }];

    let updatedSets: RoutingRuleSet[];
    if (isNewSet) {
      updatedSets = [...curSets, editingSet];
    } else {
      updatedSets = curSets.map((s) => (s.id === editingSet.id ? editingSet : s));
    }

    const updatedFormData: AppSettings = {
      ...formData,
      rule_sets: updatedSets,
      active_rule_set_id: editingSet.id,
      direct_geo_rules: editingSet.direct_rules.filter((r) => r.startsWith('geosite:') || r.startsWith('geoip:')),
      block_geo_rules: editingSet.block_rules.filter((r) => r.startsWith('geosite:') || r.startsWith('geoip:')),
      proxy_geo_rules: editingSet.proxy_rules.filter((r) => r.startsWith('geosite:') || r.startsWith('geoip:')),
    };

    setFormData(updatedFormData);
    await saveSettings(updatedFormData);
    setModalOpen(false);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const handleSave = async () => {
    await saveSettings(formData);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  return (
    <div className="flex-1 overflow-y-auto px-8 py-6 max-w-3xl mx-auto w-full">
      <div className="space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between pb-4 border-b border-[#1f2433]">
          <div>
            <h1 className="text-base font-bold text-gray-100">Settings</h1>
            <p className="text-xs text-gray-500">Configure networking, proxy modes, and system integration</p>
          </div>
          <button
            onClick={handleSave}
            className="flex items-center gap-1.5 px-4 py-2 rounded-xl bg-blue-600 hover:bg-blue-500 text-white text-xs font-semibold shadow-sm shadow-blue-500/20 transition-all"
          >
            {saved ? <Check className="w-3.5 h-3.5" /> : <Save className="w-3.5 h-3.5" />}
            <span>{saved ? 'Saved!' : 'Save Changes'}</span>
          </button>
        </div>

        {/* Section: Connection Mode */}
        <div className="bg-[#12151f] border border-[#212637] rounded-2xl p-5 space-y-4">
          <div className="flex items-center gap-2 text-xs font-semibold text-gray-200">
            <Network className="w-4 h-4 text-blue-400" />
            <span>Connection Mode</span>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            {[
              {
                id: 'system_proxy',
                title: 'System Proxy',
                desc: 'Routes browser and standard app HTTP/SOCKS traffic seamlessly.',
              },
              {
                id: 'tun_mode',
                title: 'TUN Mode',
                desc: 'Virtual network adapter (Wintun) capturing all system TCP/UDP.',
              },
              {
                id: 'proxy_only',
                title: 'Proxy Only',
                desc: 'Runs local mixed port without altering Windows system proxy.',
              },
            ].map((mode) => {
              const isSelected = formData.proxy_mode === mode.id;
              return (
                <div
                  key={mode.id}
                  onClick={() => setFormData({ ...formData, proxy_mode: mode.id as ProxyMode })}
                  className={`p-3.5 rounded-xl border cursor-pointer transition-all ${
                    isSelected
                      ? 'bg-blue-500/10 border-blue-500/40 text-blue-400'
                      : 'bg-[#0f1118] border-[#222736] text-gray-400 hover:border-[#2e3447]'
                  }`}
                >
                  <div className="text-xs font-semibold text-gray-200 mb-1">{mode.title}</div>
                  <div className="text-[11px] text-gray-400 leading-relaxed">{mode.desc}</div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Section: Network Ports */}
        <div className="bg-[#12151f] border border-[#212637] rounded-2xl p-5 space-y-4">
          <div className="flex items-center gap-2 text-xs font-semibold text-gray-200">
            <Cpu className="w-4 h-4 text-indigo-400" />
            <span>Ports & API</span>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div>
              <label className="text-[11px] font-medium text-gray-400 block mb-1.5">
                Local Mixed Port (HTTP & SOCKS5)
              </label>
              <input
                type="number"
                value={formData.mixed_port}
                onChange={(e) => setFormData({ ...formData, mixed_port: parseInt(e.target.value) || 2080 })}
                className="w-full bg-[#0b0d14] border border-[#212638] rounded-xl px-3 py-2 text-xs text-gray-200 font-mono focus:outline-none focus:border-blue-500/60"
              />
            </div>

            <div>
              <label className="text-[11px] font-medium text-gray-400 block mb-1.5">
                Clash API External Controller Port
              </label>
              <input
                type="number"
                value={formData.clash_api_port}
                onChange={(e) => setFormData({ ...formData, clash_api_port: parseInt(e.target.value) || 9090 })}
                className="w-full bg-[#0b0d14] border border-[#212638] rounded-xl px-3 py-2 text-xs text-gray-200 font-mono focus:outline-none focus:border-blue-500/60"
              />
            </div>
          </div>
        </div>

        {/* Section: Routing Policy & China Bypass */}
        <div className="bg-[#12151f] border border-[#212637] rounded-2xl p-5 space-y-4">
          <div className="flex items-center gap-2 text-xs font-semibold text-gray-200">
            <Network className="w-4 h-4 text-emerald-400" />
            <span>Routing Policy & Mainland China Bypass</span>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            {[
              {
                id: 'rule',
                title: 'Rule (Smart Bypass)',
                desc: 'Bypasses mainland China domains, China IPs, and private LAN IPs directly.',
              },
              {
                id: 'global',
                title: 'Global Proxy',
                desc: 'Routes all outbound internet traffic through the proxy node.',
              },
              {
                id: 'direct',
                title: 'Direct All',
                desc: 'Bypasses proxy for all connections without terminating the core.',
              },
            ].map((route) => {
              const isSelected = formData.routing_mode === route.id;
              return (
                <div
                  key={route.id}
                  onClick={() => setFormData({ ...formData, routing_mode: route.id })}
                  className={`p-3.5 rounded-xl border cursor-pointer transition-all ${
                    isSelected
                      ? 'bg-emerald-500/10 border-emerald-500/40 text-emerald-400'
                      : 'bg-[#0f1118] border-[#222736] text-gray-400 hover:border-[#2e3447]'
                  }`}
                >
                  <div className="text-xs font-semibold text-gray-200 mb-1">{route.title}</div>
                  <div className="text-[11px] text-gray-400 leading-relaxed">{route.desc}</div>
                </div>
              );
            })}
          </div>

          <div className="flex items-center justify-between py-2 border-t border-[#1c202d] pt-3">
            <div>
              <div className="text-xs font-medium text-gray-200">Bypass Mainland China & Private LAN</div>
              <div className="text-[11px] text-gray-500">Directly connect using GeoSite / GeoIP rulesets (geosite:cn, geoip:cn, private LAN)</div>
            </div>
            <button
              onClick={() => setFormData({ ...formData, bypass_china: !formData.bypass_china })}
              className={`w-10 h-5 rounded-full p-0.5 transition-colors ${
                formData.bypass_china ? 'bg-emerald-600' : 'bg-gray-700'
              }`}
            >
              <div
                className={`w-4 h-4 rounded-full bg-white transition-transform ${
                  formData.bypass_china ? 'translate-x-5' : 'translate-x-0'
                }`}
              />
            </button>
          </div>

          <div className="flex items-center justify-between py-2 border-t border-[#1c202d] pt-3">
            <div>
              <div className="flex items-center gap-2">
                <span className="text-xs font-medium text-gray-200">IPv6 Network Support (IPv6 支持)</span>
                <span className={`text-[10px] px-1.5 py-0.2 rounded font-mono ${
                  formData.enable_ipv6 ? 'bg-blue-500/20 text-blue-300' : 'bg-gray-800 text-gray-400'
                }`}>
                  {formData.enable_ipv6 ? 'IPv4 + IPv6 双栈' : '仅 IPv4 (防泄漏)'}
                </span>
              </div>
              <div className="text-[11px] text-gray-500">
                开启后启用 IPv4 + IPv6 双栈分流；关闭后仅解析 IPv4 并彻底拦截 IPv6 流量泄漏（推荐在多数网络环境下保持关闭以避免 DNS 污染）
              </div>
            </div>
            <button
              onClick={() => setFormData({ ...formData, enable_ipv6: !formData.enable_ipv6 })}
              className={`w-10 h-5 rounded-full p-0.5 transition-colors ${
                formData.enable_ipv6 ? 'bg-blue-600' : 'bg-gray-700'
              }`}
            >
              <div
                className={`w-4 h-4 rounded-full bg-white transition-transform ${
                  formData.enable_ipv6 ? 'translate-x-5' : 'translate-x-0'
                }`}
              />
            </button>
          </div>

          <div className="flex items-center justify-between py-2 border-t border-[#1c202d] pt-3">
            <div>
              <div className="flex items-center gap-2">
                <span className="text-xs font-medium text-gray-200">阻断 UDP 443 (Block QUIC / HTTP3)</span>
                <span className={`text-[10px] px-1.5 py-0.2 rounded font-mono ${
                  formData.block_udp_443 ? 'bg-amber-500/20 text-amber-300' : 'bg-gray-800 text-gray-400'
                }`}>
                  {formData.block_udp_443 ? '已拦截 (强制 TCP)' : '默认允许'}
                </span>
              </div>
              <div className="text-[11px] text-gray-500">
                阻断远端 UDP 443 端口流量，强制浏览器等客户端回退到稳定 TCP TLS，解决视频加载慢或网页偶发卡顿
              </div>
            </div>
            <button
              onClick={() => setFormData({ ...formData, block_udp_443: !formData.block_udp_443 })}
              className={`w-10 h-5 rounded-full p-0.5 transition-colors ${
                formData.block_udp_443 ? 'bg-amber-600' : 'bg-gray-700'
              }`}
            >
              <div
                className={`w-4 h-4 rounded-full bg-white transition-transform ${
                  formData.block_udp_443 ? 'translate-x-5' : 'translate-x-0'
                }`}
              />
            </button>
          </div>
        </div>

        {/* Section: Routing Rule Set Settings (v2rayN pattern) */}
        <div className="bg-[#12151f] border border-[#212637] rounded-2xl p-5 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-xs font-semibold text-gray-200">
              <Layers className="w-4 h-4 text-cyan-400" />
              <span>路由规则集设置 (Routing Rule Sets)</span>
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={handleOpenCreateNew}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[#1c2233] hover:bg-[#252c42] text-cyan-400 text-xs font-medium border border-cyan-500/20 hover:border-cyan-500/40 transition-all shadow-sm"
              >
                <Plus className="w-3.5 h-3.5" />
                <span>新建规则集</span>
              </button>
              <button
                type="button"
                onClick={() => handleOpenEdit()}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-blue-600/90 hover:bg-blue-600 text-white text-xs font-medium shadow-sm transition-all"
              >
                <Edit3 className="w-3.5 h-3.5" />
                <span>编辑当前规则集</span>
              </button>
            </div>
          </div>

          <div className="text-[11px] text-gray-400 leading-relaxed">
            参考 v2rayN 路由分流架构。选择生效的路由规则集，点击“编辑”可弹出窗口自定义直连、代理与广告阻断规则。
          </div>

          <div className="bg-[#0b0d14] border border-[#212638] rounded-xl p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <div className="space-y-1">
              <div className="text-[11px] text-gray-400 font-medium">当前启用的规则集</div>
              <div className="flex items-center gap-2">
                <select
                  value={formData.active_rule_set_id || 'default'}
                  onChange={(e) => handleSelectRuleSet(e.target.value)}
                  className="bg-[#141824] border border-[#2d354d] text-gray-100 text-xs rounded-lg px-3 py-1.5 font-medium focus:outline-none focus:border-cyan-500 min-w-[160px]"
                >
                  {ruleSets.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name} {s.id === 'default' ? '(默认)' : ''}
                    </option>
                  ))}
                </select>
                <span className="text-[11px] text-emerald-400 bg-emerald-500/10 border border-emerald-500/20 px-2 py-0.5 rounded-md">
                  生效中
                </span>
              </div>
            </div>

            <div className="flex items-center gap-4 text-xs font-mono text-gray-400 bg-[#12151f] px-3.5 py-2 rounded-lg border border-[#1f2436]">
              <div>
                <span className="text-gray-500 text-[10px] block">直连规则</span>
                <span className="text-emerald-400 font-semibold">{activeRuleSet.direct_rules?.length || 0}</span>
              </div>
              <div className="w-[1px] h-6 bg-[#212638]" />
              <div>
                <span className="text-gray-500 text-[10px] block">代理规则</span>
                <span className="text-sky-400 font-semibold">{activeRuleSet.proxy_rules?.length || 0}</span>
              </div>
              <div className="w-[1px] h-6 bg-[#212638]" />
              <div>
                <span className="text-gray-500 text-[10px] block">广告拦截</span>
                <span className="text-rose-400 font-semibold">{activeRuleSet.block_rules?.length || 0}</span>
              </div>
            </div>
          </div>
        </div>

        {/* Section: Security & DNS */}
        <div className="bg-[#12151f] border border-[#212637] rounded-2xl p-5 space-y-4">
          <div className="flex items-center gap-2 text-xs font-semibold text-gray-200">
            <Shield className="w-4 h-4 text-emerald-400" />
            <span>Security & DNS Mode</span>
          </div>

          <div className="flex items-center justify-between py-2 border-b border-[#1c202d]">
            <div>
              <div className="text-xs font-medium text-gray-200">Kill Switch</div>
              <div className="text-[11px] text-gray-500">Block internet traffic if connection drops unexpectedly</div>
            </div>
            <button
              onClick={() => setFormData({ ...formData, kill_switch: !formData.kill_switch })}
              className={`w-10 h-5 rounded-full p-0.5 transition-colors ${
                formData.kill_switch ? 'bg-blue-600' : 'bg-gray-700'
              }`}
            >
              <div
                className={`w-4 h-4 rounded-full bg-white transition-transform ${
                  formData.kill_switch ? 'translate-x-5' : 'translate-x-0'
                }`}
              />
            </button>
          </div>

          <div className="flex items-center justify-between py-2">
            <div>
              <div className="text-xs font-medium text-gray-200">Remote DNS Resolution</div>
              <div className="text-[11px] text-gray-500">Encrypted DNS over HTTPS / TLS to prevent DNS Leaks</div>
            </div>
            <select
              value={formData.dns_mode}
              onChange={(e) => setFormData({ ...formData, dns_mode: e.target.value })}
              className="bg-[#0b0d14] border border-[#212638] rounded-xl px-3 py-1.5 text-xs text-gray-200 focus:outline-none focus:border-blue-500/60"
            >
              <option value="auto">Automatic (Cloudflare 1.1.1.1 DoH)</option>
              <option value="google">Google DNS (8.8.8.8 DoH)</option>
              <option value="aliyun">Alibaba Cloud DNS (223.5.5.5)</option>
            </select>
          </div>
        </div>

        {/* Core details footer */}
        <div className="text-center py-2 text-[11px] text-gray-600 font-mono">
          Engine: sing-box 1.14.0 (Windows x64 with Wintun 0.14.1) · Job Object Protected
        </div>
      </div>

      {/* Rule Set Management Modal (v2rayN style) */}
      {modalOpen && (
        <div className="fixed inset-0 bg-black/75 backdrop-blur-sm z-50 flex items-center justify-center p-4">
          <div className="bg-[#12151f] border border-[#2d354d] rounded-2xl w-full max-w-2xl max-h-[90vh] flex flex-col shadow-2xl overflow-hidden animate-in fade-in duration-200">
            {/* Modal Header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-[#212638] bg-[#0e111a]">
              <div className="flex items-center gap-2">
                <Layers className="w-4 h-4 text-cyan-400" />
                <h2 className="text-sm font-bold text-gray-100">
                  {isNewSet ? '新建路由规则集' : `编辑路由规则集: ${editingSet.name}`}
                </h2>
              </div>
              <button
                onClick={() => setModalOpen(false)}
                className="p-1 rounded-lg text-gray-400 hover:text-gray-100 hover:bg-gray-800 transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Modal Body */}
            <div className="p-6 overflow-y-auto space-y-4 flex-1">
              {/* Rule set switcher & actions within modal */}
              <div className="flex flex-wrap items-center justify-between gap-2 p-2.5 bg-[#0b0d14] rounded-xl border border-[#212638]">
                <div className="flex items-center gap-2">
                  <span className="text-[11px] text-gray-400">切换规则集:</span>
                  <select
                    value={isNewSet ? '__new__' : editingSet.id}
                    onChange={(e) => {
                      if (e.target.value === '__new__') {
                        handleOpenCreateNew();
                      } else {
                        const target = ruleSets.find((s) => s.id === e.target.value);
                        if (target) handleOpenEdit(target);
                      }
                    }}
                    className="bg-[#141824] border border-[#2d354d] text-gray-200 text-xs rounded-lg px-2.5 py-1"
                  >
                    {ruleSets.map((s) => (
                      <option key={s.id} value={s.id}>
                        {s.name} {s.id === 'default' ? '(默认)' : ''}
                      </option>
                    ))}
                    {isNewSet && <option value="__new__">[新建中] {editingSet.name}</option>}
                  </select>
                </div>

                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={handleOpenCreateNew}
                    className="flex items-center gap-1 text-[11px] text-cyan-400 hover:text-cyan-300 font-medium px-2 py-1 rounded-lg hover:bg-cyan-500/10 transition-colors"
                  >
                    <Plus className="w-3 h-3" />
                    <span>新建规则集</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setEditingSet({
                        ...editingSet,
                        direct_rules: ['geosite:cn', 'geoip:cn', 'geosite:private', 'geoip:private'],
                        proxy_rules: [],
                        block_rules: ['geosite:category-ads-all'],
                      });
                    }}
                    className="flex items-center gap-1 text-[11px] text-gray-400 hover:text-gray-200 px-2 py-1 rounded-lg hover:bg-gray-800 transition-colors"
                    title="恢复推荐默认规则"
                  >
                    <RotateCcw className="w-3 h-3" />
                    <span>恢复推荐</span>
                  </button>
                  {!isNewSet && editingSet.id !== 'default' && (
                    <button
                      type="button"
                      onClick={() => {
                        if (confirm(`确定要删除规则集 "${editingSet.name}" 吗？`)) {
                          const updatedSets = ruleSets.filter((s) => s.id !== editingSet.id);
                          const fallbackId = updatedSets[0]?.id || 'default';
                          const updated = {
                            ...formData,
                            rule_sets: updatedSets,
                            active_rule_set_id: formData.active_rule_set_id === editingSet.id ? fallbackId : formData.active_rule_set_id,
                          };
                          setFormData(updated);
                          saveSettings(updated);
                          setModalOpen(false);
                        }
                      }}
                      className="flex items-center gap-1 text-[11px] text-rose-400 hover:text-rose-300 px-2 py-1 rounded-lg hover:bg-rose-500/10 transition-colors"
                    >
                      <Trash2 className="w-3 h-3" />
                      <span>删除</span>
                    </button>
                  )}
                </div>
              </div>

              {/* Rule Set Name */}
              <div>
                <label className="text-[11px] font-medium text-gray-300 block mb-1">
                  规则集名称
                </label>
                <input
                  type="text"
                  value={editingSet.name}
                  onChange={(e) => setEditingSet({ ...editingSet, name: e.target.value })}
                  placeholder="例如：默认 / 游戏分流 / 学习办公"
                  className="w-full bg-[#0b0d14] border border-[#212638] rounded-xl px-3 py-2 text-xs text-gray-200 focus:outline-none focus:border-cyan-500"
                />
              </div>

              {/* Direct Rules */}
              <div>
                <div className="flex items-center justify-between mb-1">
                  <label className="text-[11px] font-medium text-emerald-400">
                    国内与局域网直连规则 (Direct)
                  </label>
                  <span className="text-[10px] text-gray-500 font-mono">
                    {editingSet.direct_rules.length} 条
                  </span>
                </div>
                <p className="text-[10px] text-gray-500 mb-1.5">
                  每行一条。支持 <code>geosite:cn</code>, <code>geoip:cn</code>, <code>geosite:private</code>, <code>geoip:private</code> 或普通域名/CIDR（如 <code>local.dev</code>, <code>192.168.1.0/24</code>）。
                </p>
                <textarea
                  rows={4}
                  value={editingSet.direct_rules.join('\n')}
                  onChange={(e) =>
                    setEditingSet({
                      ...editingSet,
                      direct_rules: e.target.value.split('\n').map((l) => l.trim()).filter(Boolean),
                    })
                  }
                  placeholder="geosite:cn&#10;geoip:cn&#10;geosite:private&#10;geoip:private"
                  className="w-full bg-[#0b0d14] border border-[#212638] rounded-xl px-3 py-2 text-xs text-emerald-300 font-mono focus:outline-none focus:border-emerald-500/60 leading-relaxed"
                />
              </div>

              {/* Proxy Rules */}
              <div>
                <div className="flex items-center justify-between mb-1">
                  <label className="text-[11px] font-medium text-sky-400">
                    海外代理规则 (Proxy Outbound)
                  </label>
                  <span className="text-[10px] text-gray-500 font-mono">
                    {editingSet.proxy_rules.length} 条
                  </span>
                </div>
                <p className="text-[10px] text-gray-500 mb-1.5">
                  强制走节点代理的规则。支持 <code>geosite:google</code>, <code>openai.com</code>, <code>1.1.1.1/32</code> 等。
                </p>
                <textarea
                  rows={3}
                  value={editingSet.proxy_rules.join('\n')}
                  onChange={(e) =>
                    setEditingSet({
                      ...editingSet,
                      proxy_rules: e.target.value.split('\n').map((l) => l.trim()).filter(Boolean),
                    })
                  }
                  placeholder="geosite:google&#10;openai.com"
                  className="w-full bg-[#0b0d14] border border-[#212638] rounded-xl px-3 py-2 text-xs text-sky-300 font-mono focus:outline-none focus:border-sky-500/60 leading-relaxed"
                />
              </div>

              {/* Block Rules */}
              <div>
                <div className="flex items-center justify-between mb-1">
                  <label className="text-[11px] font-medium text-rose-400">
                    广告拦截 / 阻断规则 (Block / Reject)
                  </label>
                  <span className="text-[10px] text-gray-500 font-mono">
                    {editingSet.block_rules.length} 条
                  </span>
                </div>
                <p className="text-[10px] text-gray-500 mb-1.5">
                  匹配流量直接丢弃拦截。默认 <code>geosite:category-ads-all</code>。
                </p>
                <textarea
                  rows={2}
                  value={editingSet.block_rules.join('\n')}
                  onChange={(e) =>
                    setEditingSet({
                      ...editingSet,
                      block_rules: e.target.value.split('\n').map((l) => l.trim()).filter(Boolean),
                    })
                  }
                  placeholder="geosite:category-ads-all"
                  className="w-full bg-[#0b0d14] border border-[#212638] rounded-xl px-3 py-2 text-xs text-rose-300 font-mono focus:outline-none focus:border-rose-500/60 leading-relaxed"
                />
              </div>
            </div>

            {/* Modal Footer */}
            <div className="flex items-center justify-end gap-2.5 px-6 py-4 border-t border-[#212638] bg-[#0e111a]">
              <button
                type="button"
                onClick={() => setModalOpen(false)}
                className="px-4 py-2 rounded-xl bg-[#1c2233] hover:bg-[#252c42] text-gray-300 text-xs font-medium transition-colors"
              >
                取消
              </button>
              <button
                type="button"
                onClick={handleSaveRuleSet}
                className="flex items-center gap-1.5 px-5 py-2 rounded-xl bg-blue-600 hover:bg-blue-500 text-white text-xs font-semibold shadow-sm shadow-blue-500/20 transition-all"
              >
                <Save className="w-3.5 h-3.5" />
                <span>保存并应用</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
