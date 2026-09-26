import { create } from 'zustand';
import { api } from '../services/api';
import { invoke } from '@tauri-apps/api/core';
import { ActiveTab, AppSettings, ConnectionStatus, LivenessProgress, ProbeOutcome, ProxyChain, RelayRanking, Subscription, TrafficStats, UnifiedNode } from '../types';


export interface InspectReportItem {
  id: string;
  oldName: string;
  newName: string;
  oldCountry: string;
  newCountry: string;
  oldLatency: number | null;
  newLatency: number | null;
  oldSpeed: number | null;
  newSpeed: number | null;
}

export interface InspectReport {
  items: InspectReportItem[];
}

interface AppStore {
  status: ConnectionStatus;
  connectedNode: UnifiedNode | null;
  traffic: TrafficStats;
  nodes: UnifiedNode[];
  subscriptions: Subscription[];
  chains: ProxyChain[];
  connectedChainId: string | null;
  selectedNodeId: string | null;
  settings: AppSettings;
  activeTab: ActiveTab;
  isTestingAll: boolean;
  searchQuery: string;
  selectedProtocol: string;
  selectedGroup: string;
  filterFavorite: boolean;
  sortBySpeed: boolean;
  sortMode: 'speed' | 'latency' | 'default';
  isTestingAllSpeed: boolean;
  testingLatencyIds: string[];
  testingSpeedIds: string[];
  testingChainIds: string[];
  errorMessage: string | null;
  tunnelStage: string | null;
  crashReport: string | null;
  dismissCrashReport: () => void;
  inspectProgress: { current: number; total: number; status: string } | null;
  inspectReport: InspectReport | null;
  closeInspectReport: () => void;

  // CF Auth
  machineId: string | null;
  authDisplayText: string | null;
  isAuthorized: boolean;
  residentialSubUrl: string | null;
  loadResidentialNodes: () => Promise<UnifiedNode[]>;

  // Actions
  init: () => Promise<void>;
  setActiveTab: (tab: ActiveTab) => void;
  setSearchQuery: (q: string) => void;
  setSelectedProtocol: (p: string) => void;
  setSelectedGroup: (g: string) => void;
  setFilterFavorite: (f: boolean) => void;
  setSortBySpeed: (s: boolean) => void;
  setSortMode: (mode: 'speed' | 'latency' | 'default') => void;
  setSelectedNodeId: (id: string | null) => void;
  setErrorMessage: (msg: string | null) => void;
  relayEnabled: boolean;
  selectedRelayNodeId: string;
  relayCandidates: UnifiedNode[];
  preferredRelay: RelayRanking | null;
  isRankingRelays: boolean;
  // 后台真连接测活进度,键是名单名(VPNGate / Residential)
  livenessProgress: Record<string, LivenessProgress>;
  // 单节点"测活"的结论,键是节点 id。必须存在这里而不是面板组件的 state 里:
  // 用户在等待时切走标签页,组件卸载后回来的 setState 会落空,回来就又是一张
  // "不知道有没有测过"的卡片。
  probeOutcomes: Record<string, ProbeOutcome>;
  // 刚刚那一次单节点测活。卡片会被重排送走(判死的沉底、列表重新拉取),所以这句
  // 话必须另外钉在面板顶部 —— 用户投诉的"提示一闪而过"就是这个意思。
  lastProbe: { nodeId: string; nodeName: string; outcome: ProbeOutcome } | null;
  setProbeOutcome: (nodeId: string, outcome: ProbeOutcome, nodeName?: string) => void;
  dismissLastProbe: () => void;
  // 单节点测活之后只改这一行,不重拉整份清单:全量刷新 + 重排会把刚测完的那张
  // 卡片送到看不见的地方,用户回来只看到一片"未测"。
  applyMeasuredNode: (node: UnifiedNode) => void;
  // 某台中转被测活判定不可用:中转栏上关于它的"已实测/自动选用"必须当场作废。
  relayConvicted: (relayName: string) => void;
  fetchRelayCandidates: () => Promise<void>;
  rankRelays: () => Promise<void>;
  // 中转栏"刷新"这一下的结论,必须成句显示在栏上。从前这个按钮只重新拉一次候选
  // 列表,按完和没按一样 —— 而它该回答的是"现在哪台中转能用"。
  relayRank: { message: string; ok: boolean } | null;
  refreshRelays: () => Promise<void>;
  setRelayEnabled: (enabled: boolean) => void;
  setSelectedRelayNodeId: (id: string) => void;

  connect: (nodeId?: string, relayIdOverride?: string | null) => Promise<void>;
  disconnect: () => Promise<void>;
  refreshNodes: () => Promise<void>;
  refreshSubscriptions: () => Promise<void>;
  restoreDefaultSubscriptions: () => Promise<void>;
  updateAllSubscriptions: () => Promise<void>;
  applyCustomSubscription: (customUrl: string) => Promise<void>;
  addSubscription: (name: string, url: string) => Promise<boolean>;
  editSubscription: (id: string, name: string, url: string) => Promise<boolean>;
  deleteSubscription: (id: string) => Promise<void>;
  updateSubViaProxy: boolean;
  setUpdateSubViaProxy: (val: boolean) => void;
  updateSubscription: (id: string) => Promise<void>;
  autoRefreshDefault: () => Promise<void>;
  checkAuth: (forceRemote?: boolean) => Promise<boolean>;
  testLatency: (id: string) => Promise<void>;
  testSpeed: (id: string) => Promise<void>;
  testAllNodes: () => Promise<void>;
  testAllSpeeds: () => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
  deleteNode: (id: string) => Promise<void>;
  importLink: (link: string) => Promise<boolean>;
  saveSettings: (settings: AppSettings) => Promise<void>;

  // Chained Proxy Actions
  refreshChains: () => Promise<void>;
  addChain: (chain: { name: string; remarks?: string; node_ids: string[] }) => Promise<boolean>;
  updateChain: (chain: ProxyChain) => Promise<boolean>;
  deleteChain: (id: string) => Promise<void>;
  connectChain: (chainId: string) => Promise<void>;
  connectSmartGroup: (nodeIds: string[]) => Promise<void>;
  testChainLatency: (chainId: string) => Promise<void>;
  testAllChains: () => Promise<void>;
  startDeepInspection: (nodes: UnifiedNode[]) => Promise<void>;
}
const JWT_SECRET = "NARCISSUS_AURA_SUPER_SECRET_KEY_2026";

async function verifyJWT(token: string) {
  try {
    const parts = token.split('.');
    if (parts.length !== 3) return null;
    const [header, body, sigBase64Url] = parts;
    const data = `${header}.${body}`;
    
    const enc = new TextEncoder();
    const key = await crypto.subtle.importKey(
      "raw", enc.encode(JWT_SECRET),
      { name: "HMAC", hash: "SHA-256" },
      false, ["verify"]
    );
    
    // Base64URL to regular Base64
    let sigBase64 = sigBase64Url.replace(/-/g, '+').replace(/_/g, '/');
    while (sigBase64.length % 4) {
      sigBase64 += '=';
    }
    
    const sigBytes = Uint8Array.from(atob(sigBase64), c => c.charCodeAt(0));
    const isValid = await crypto.subtle.verify("HMAC", key, sigBytes, enc.encode(data));
    
    if (!isValid) return null;
    
    // Decode body
    let bodyBase64 = body.replace(/-/g, '+').replace(/_/g, '/');
    while (bodyBase64.length % 4) {
      bodyBase64 += '=';
    }
    const decodedBody = JSON.parse(decodeURIComponent(escape(atob(bodyBase64))));
    return decodedBody;
  } catch (e) {
    console.error('JWT verify error', e);
    return null;
  }
}

let currentConnectSeq = 0;

// Android tunnel handshake stages reported by VpnService (vpn_status file →
// core:vpn-stage event). Every consent_* stage funnels into the SAME
// instruction: grant consent in the SYSTEM VPN settings page. MIUI swallows
// every consent dialog raised by our own process (v0.2.85-87 field
// evidence), but the dialog raised BY Settings itself cannot be blocked —
// and once consent lands, prepare() returns null forever, so later connects
// need no dialog at all. Rust keeps re-signalling the service every ~4s for
// 3 minutes: the tunnel builds automatically the instant consent exists.
function translateVpnStage(raw: string): string {
  const settingsConsent =
    '请打开手机「设置 → 连接与共享 → VPN」（部分机型：设置 → 更多连接 → VPN，或 WLAN 设置页 → VPN），' +
    '在「可使用的 VPN 应用」中点击「Narcissus Aura」，弹出授权页时点「允许」（建议勾选「不再询问」），' +
    '再回到本应用——隧道会自动继续建立，无需再点任何东西';
  if (raw === 'service_starting') return '正在启动隧道服务...';
  if (raw === 'consent_required')
    return '正在弹出系统 VPN 授权窗口，请在弹窗中点击「允许」，隧道会自动继续建立，无需再点连接；若 3 秒后没有弹窗：' + settingsConsent;
  if (raw === 'consent_dialog_opened') return '系统授权弹窗已打开，请在弹窗中点击「允许」';
  if (raw.startsWith('consent_activity_failed')) return settingsConsent;
  if (raw === 'consent_no_activity') return settingsConsent;
  if (raw === 'consent_notify_posted') return settingsConsent + '；也可点击通知栏「需要允许 VPN 连接」那条通知完成授权';
  if (raw === 'consent_notify_blocked') return '通知入口被系统禁用（不影响下面的主路径）：' + settingsConsent;
  if (raw === 'consent_denied') return settingsConsent + '。本页会保持「正在连接」最多 3 分钟等你的授权落地；这是部分 MIUI 对本应用的弹窗拦截所致，在系统设置里允许一次后，以后直接点「连接」即可';
  if (raw.startsWith('consent_overlay_failed')) return settingsConsent;
  if (raw.startsWith('consent_notify_failed')) return settingsConsent;
  if (raw === 'establishing') return '正在建立 VPN 隧道...';
  if (raw.startsWith('established')) return '隧道已建立，正在启动代理核心...';
  if (raw.startsWith('establish_failed')) return '系统拒绝创建 VPN 隧道，请重试或在系统设置中检查 VPN 权限';
  if (raw.startsWith('service_start_failed')) return `隧道服务启动失败：${raw.substring('service_start_failed:'.length)}`;
  if (raw.startsWith('fgs_start_failed')) return '系统阻止了前台通知，隧道服务无法启动。请在系统设置 → 通知中允许「Narcissus Aura」显示通知后重试';
  if (raw === 'standby') return '隧道服务待机中...';
  return raw;
}

/** IPC 失败原因必须落到面板上,不能只进 console。 */
const errText = (e: unknown): string => (e instanceof Error ? e.message : String(e));

/** 实测带宽是字节/秒,换算成看得懂的 MB/s */
const formatBandwidth = (bytesPerSecond?: number | null): string | null => {
  if (!bytesPerSecond || bytesPerSecond <= 0) return null;
  return `${(bytesPerSecond / 1048576).toFixed(1)}MB/s`;
};

const describeRelay = (n: {
  name: string;
  country_code?: string;
  latency_ms?: number | null;
  speed_bps?: number | null;
}): string => {
  const detail = [
    n.country_code,
    n.latency_ms != null ? `${n.latency_ms}ms` : null,
    formatBandwidth(n.speed_bps),
  ]
    .filter(Boolean)
    .join(' · ');
  return `「${n.name}」${detail ? ` (${detail})` : ''}`;
};

export const useAppStore = create<AppStore>((set, get) => ({
  status: 'disconnected',
  connectedNode: null,
  traffic: {
    upload_bytes: 0,
    download_bytes: 0,
    upload_speed: 0,
    download_speed: 0,
    uptime_seconds: 0,
  },
  nodes: [],
  subscriptions: [],
  chains: [],
  connectedChainId: null,
  selectedNodeId: null,
  settings: {
    proxy_mode: 'system_proxy',
    mixed_port: 2080,
    clash_api_port: 9090,
    auto_connect: false,
    auto_start: false,
    kill_switch: false,
    dns_mode: 'auto',
    bypass_china: true,
    enable_ipv6: false,
    routing_mode: 'rule',
    custom_direct_rules: [],
    custom_proxy_rules: [],
    theme: 'dark',
  },
  activeTab: 'dashboard',
  isTestingAll: false,
  isTestingAllSpeed: false,
  searchQuery: '',
  selectedProtocol: 'all',
  selectedGroup: 'all',
  filterFavorite: false,
  sortBySpeed: false,
  sortMode: 'speed',
  testingLatencyIds: [],
  testingSpeedIds: [],
  testingChainIds: [],
  errorMessage: null,
  tunnelStage: null,
  crashReport: null,
  inspectProgress: null,
  inspectReport: null,
  
  machineId: null,
  authDisplayText: null,
  isAuthorized: false,
  residentialSubUrl: null,
  loadResidentialNodes: async () => {
    const url = get().residentialSubUrl;
    if (!url || !url.trim()) {
      return [];
    }
    try {
      const fetched = await api.fetchResidentialNodes(url.trim());
      await get().refreshNodes();
      return fetched;
    } catch (e) {
      console.error('Failed to load residential nodes:', e);
      return [];
    }
  },
  updateSubViaProxy: true,
  setUpdateSubViaProxy: (val) => set({ updateSubViaProxy: val }),
  relayEnabled: false,
  selectedRelayNodeId: 'auto',
  relayCandidates: [],
  preferredRelay: null,
  isRankingRelays: false,
  livenessProgress: {},
  // 单节点测活的结论存在全局 store,不放面板组件的 state 里:等待期间用户切走
  // 标签页,组件卸载,回来的 setState 就落空 —— 那张卡片又变成"不知道测没测过"。
  probeOutcomes: {},
  lastProbe: null,
  setRelayEnabled: (enabled) => set({ relayEnabled: enabled }),
  setSelectedRelayNodeId: (id) => set({ selectedRelayNodeId: id }),
  fetchRelayCandidates: async () => {
    try {
      const relayCandidates = await api.getRelayCandidates();
      set({ relayCandidates });
    } catch (e) {
      console.error('Failed to fetch relay candidates:', e);
      // 候选列表拉不动,面板上剩下的就是一个空下拉框 —— 和按钮坏了没法区分。
      set({ relayRank: { message: `中转候选没能刷新：${errText(e)}`, ok: false } });
    }
  },
  rankRelays: async () => {
    if (get().isRankingRelays) {
      set({ relayRank: { message: '中转实测正在进行中，这一轮测完会自己报结果', ok: false } });
      return;
    }
    set({ isRankingRelays: true, relayRank: null });
    try {
      const ranking = await api.rankRelays();
      if (!ranking) {
        set({ relayRank: { message: '实测没能进行：核心没有返回任何结果，请查看日志', ok: false } });
        return;
      }
      set({ preferredRelay: ranking });
      // 测得的延迟/带宽写回在节点记录上,重新拉一次候选才有真实数字
      await get().fetchRelayCandidates();
      const row = ranking.preferred_id
        ? get().relayCandidates.find((n) => n.id === ranking.preferred_id)
        : undefined;
      if (ranking.preferred_id) {
        // 实测过的胜者就是"当前可用的中转",所以这一轮结束后选择框必须落在它身上。
        // 'auto' 就是它:后端读的就是刚写下的 preferred_relay_id。
        set({
          selectedRelayNodeId: 'auto',
          relayRank: {
            message: `已实测并自动选用：${describeRelay(
              row ?? {
                name: ranking.preferred_name ?? ranking.preferred_id,
                latency_ms: ranking.latency_ms,
                speed_bps: ranking.speed_bps,
              }
            )}${ranking.aborted ? `（真实连接占用了设备，本轮只测了 ${ranking.tested} 台）` : ''}`,
            ok: true,
          },
        });
      } else if (ranking.tested === 0) {
        set({ relayRank: { message: '实测没能进行：没有可拨号的中转候选，请先导入订阅（或让本地代理监听 10808/7890）', ok: false } });
      } else if (ranking.aborted) {
        set({ relayRank: { message: `真实连接正在占用设备，实测在 ${ranking.tested} 台后停下，还没测出可用中转`, ok: false } });
      } else {
        set({ relayRank: { message: `${ranking.tested} 台候选中转全部拨不通：暂时没有可用中转，请更新订阅或换一批节点`, ok: false } });
      }
    } catch (e) {
      console.error('Failed to rank relay candidates:', e);
      set({ relayRank: { message: `实测没能进行：${errText(e)}`, ok: false } });
    } finally {
      set({ isRankingRelays: false });
    }
  },
  relayRank: null,
  refreshRelays: async () => {
    // 刷新 = 拉最新候选 + 真实建联实测 + 把选择落到可用那台上。只做第一件事的
    // 按钮按完和没按一样,用户要的"刷新有价值"就是这个意思。
    await get().fetchRelayCandidates();
    await get().rankRelays();
  },

  init: async () => {
    try {
      const [status, connectedNode, nodes, subscriptions, settings, relayCandidates, chains, connectedChainId] = await Promise.all([
        api.getConnectionStatus(),
        api.getConnectedNode(),
        api.getNodes(),
        api.getSubscriptions(),
        api.getSettings(),
        api.getRelayCandidates().catch(() => []),
        api.getChains().catch(() => []),
        api.getConnectedChain().catch(() => null),
      ]);

      const selectedId = connectedNode ? connectedNode.id : (nodes.length > 0 ? nodes[0].id : null);

      set({
        status,
        connectedNode,
        nodes,
        subscriptions,
        settings,
        relayCandidates,
        chains,
        connectedChainId,
        selectedNodeId: selectedId,
      });

      // Immediately verify cached token on startup so UI states (like Residential tab) populate instantly
      const authOk = await get().checkAuth(false).catch((e) => {
        console.error('[init] checkAuth failed:', e);
        return false;
      });

      // Only auto refresh subscription if auth succeeded
      if (authOk) {
        get().autoRefreshDefault().catch(console.error);
      }

      // Listen for background state events
      api.onStatusChanged((newStatus) => {
        set({ status: newStatus });
        if (newStatus !== 'connecting') {
          set({ tunnelStage: null });
        }
        if (newStatus === 'disconnected') {
          set({
            connectedNode: null,
            connectedChainId: null,
            traffic: {
              upload_bytes: 0,
              download_bytes: 0,
              upload_speed: 0,
              download_speed: 0,
              uptime_seconds: 0,
            },
          });
        }
      });

      api.onTrafficTick((stats) => {
        set({ traffic: stats });
      });

      api.onVpnStage((stage) => {
        set({ tunnelStage: translateVpnStage(stage) });
      }).catch(() => {});

      // 启动后后台的中转优选结果:谁被实测出来了
      api.onRelayPreferred((ranking) => {
        set({ preferredRelay: ranking });
        get().fetchRelayCandidates().catch(() => {});
      }).catch(() => {});

      // 后台采集/测活写回的节点列表(启动流水线会分批发这个)
      api.onNodesUpdated(() => {
        get().refreshNodes().catch(() => {});
      }).catch(() => {});

      // 真连接测活:后台每拨一个节点就推一次进度(计数会动),但只有它把结论
      // 写回节点库时(persisted)才值得重拉整份列表 —— 一轮上百次拨号,不能每次
      // 都全量刷新。
      api.onNodesLiveness((progress) => {
        set({
          // at 是本地盖的时间戳:一轮在跑时每个节点都有一拍,前端靠它判断
          // "这一轮已经没动静了",而不是把按钮永久灰着。
          livenessProgress: { ...get().livenessProgress, [progress.group]: { ...progress, at: Date.now() } },
        });
        if (progress.persisted || !progress.running) {
          get().refreshNodes().catch(() => {});
        }
        // 这一轮里某台中转被实测判死了。栏上那句绿色的"已实测并自动选用「X」"
        // 必须在这一拍改掉 —— 同屏两个相反结论就是用户说的"中转没问题,你在撒谎"。
        if (progress.relay_retired) {
          get().relayConvicted(progress.relay_retired);
        }
      }).catch(() => {});

      // Surface a captured crash from the previous run (Android only; the
      // backend deletes the log after reading so it shows exactly once).
      api.getCrashReport().then((report) => {
        if (report) {
          console.error('Previous run crashed:\n', report);
          set({ crashReport: report });
        }
      }).catch(() => {});
    } catch (e: any) {
      console.error('Init failed:', e);
      set({ errorMessage: String(e) });
    }
  },

  setActiveTab: (tab) => set({ activeTab: tab, errorMessage: null }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  setSelectedProtocol: (selectedProtocol) => set({ selectedProtocol }),
  setSelectedGroup: (selectedGroup) => set({ selectedGroup }),
  setFilterFavorite: (filterFavorite) => set({ filterFavorite }),
  setSortBySpeed: (sortBySpeed) => set({ sortBySpeed, sortMode: sortBySpeed ? 'speed' : 'default' }),
  setSortMode: (sortMode) => set({ sortMode, sortBySpeed: sortMode === 'speed' }),
  setSelectedNodeId: (selectedNodeId) => set({ selectedNodeId }),
  setErrorMessage: (errorMessage) => set({ errorMessage }),
  setProbeOutcome: (nodeId, outcome, nodeName) => {
    const stamped = { ...outcome, at: Date.now() };
    set({
      probeOutcomes: { ...get().probeOutcomes, [nodeId]: stamped },
      lastProbe: { nodeId, nodeName: nodeName ?? '', outcome: stamped },
    });
    // 单节点测活也能把中转定罪。那一刻中转栏上"已实测并自动选用「X」"就是假的,
    // 必须和整组测活一样当场作废 —— 两处说法相反,用户只会认为 app 在编。
    if (outcome.verdict === 'relay-dead') {
      // 换过一次机时这句话里有两个名字:「A」「B」。锚在"连拨"上,别把句中的
      // 「测活」也当成中转的名字。
      const named = /中转节点「(.+?)」连拨/.exec(outcome.message);
      if (named) for (const name of named[1].split('」「')) get().relayConvicted(name);
    }
  },
  dismissLastProbe: () => set({ lastProbe: null }),
  applyMeasuredNode: (node) =>
    set({ nodes: get().nodes.map((n) => (n.id === node.id ? node : n)) }),
  relayConvicted: (relayName) => {
    const st = get();
    // 整组测活之后每一拍都带着"本轮判死了哪台中转",同一个名字不能反复触发重拉候选。
    if (st.relayRank?.message.includes(`中转「${relayName}」已被测活判定不可用`)) return;
    // 只作废"关于这一台"的说法:另一台中转的实测结论还是真的。
    const aboutIt = st.preferredRelay?.preferred_name === relayName;
    const chosen = st.relayCandidates.find((n) => n.name === relayName);
    set({
      preferredRelay: aboutIt ? null : st.preferredRelay,
      selectedRelayNodeId:
        st.selectedRelayNodeId !== 'auto' && chosen && st.selectedRelayNodeId === chosen.id
          ? 'auto'
          : st.selectedRelayNodeId,
      relayRank: {
        message: `中转「${relayName}」已被测活判定不可用${aboutIt ? '，自动优选已取消' : ''}，请点中转栏的刷新重新实测一台`,
        ok: false,
      },
    });
    // 候选要被重新拉一次:那台中转现在带着"实测不通"的记录,下拉框里看得见。
    st.fetchRelayCandidates().catch(() => {});
  },
  dismissCrashReport: () => set({ crashReport: null }),
  closeInspectReport: () => set({ inspectReport: null }),

  connect: async (overrideId, relayIdOverride) => {
    const connectSeq = ++currentConnectSeq;
    // Auth Check
    const isAuth = await get().checkAuth();
    if (!isAuth) {
      if (connectSeq === currentConnectSeq) {
        const authText = get().authDisplayText || '请联系服务商授权';
        set({ errorMessage: authText });
      }
      return;
    }

    const state = get();
    const targetId = overrideId || state.selectedNodeId;
    if (!targetId) {
      if (connectSeq === currentConnectSeq) {
        set({ errorMessage: 'Please select a server first.' });
      }
      return;
    }

    const connectingNode = state.nodes.find((n) => n.id === targetId) || null;
    const isResidential = connectingNode?.group === 'Residential';
    set({ status: 'connecting', selectedNodeId: targetId, connectedNode: connectingNode, connectedChainId: null, errorMessage: null, tunnelStage: null });

    let relayParam: string | null = null;
    if (relayIdOverride !== undefined) {
      relayParam = relayIdOverride;
    } else if (state.relayEnabled || isResidential) {
      relayParam = state.selectedRelayNodeId || 'auto';
    } else {
      relayParam = 'none';
    }

    try {
      await api.connect(targetId, relayParam);
      if (connectSeq !== currentConnectSeq) return;
      const connected = get().nodes.find((n) => n.id === targetId) || connectingNode;
      set({ status: 'connected', connectedNode: connected, selectedNodeId: targetId, connectedChainId: null });
    } catch (e: any) {
      if (connectSeq !== currentConnectSeq) return;
      console.error('Connect failed:', e);
      let errStr = typeof e === 'string' ? e : (e?.message || JSON.stringify(e));
      if (errStr.includes('cancelled') || errStr.includes('abort') || errStr.includes('终止')) {
        set({ status: 'disconnected', connectedNode: null });
        return;
      }
      // Relay-side failure ([中转节点不可用], tagged by the Rust probe): walk
      // through the other relay candidates before surfacing an error — the
      // exit node was never even tested, so giving up here wastes a working
      // subscription node further down the latency ranking.
      if (errStr.includes('[中转节点不可用]') && relayParam !== 'none') {
        let candidates: typeof state.nodes = [];
        try {
          candidates = await api.getRelayCandidates();
        } catch {}
        if (connectSeq !== currentConnectSeq) return;
        const failedId = relayParam !== 'auto' ? relayParam : candidates[0]?.id;
        const queue = candidates.filter((c) => c.id !== failedId).slice(0, 4);
        for (const cand of queue) {
          set({ status: 'connecting', tunnelStage: null });
          try {
            await api.connect(targetId, cand.id);
            if (connectSeq !== currentConnectSeq) return;
            const connected = get().nodes.find((n) => n.id === targetId) || connectingNode;
            set({
              status: 'connected',
              connectedNode: connected,
              selectedNodeId: targetId,
              connectedChainId: null,
              selectedRelayNodeId: cand.id,
            });
            return;
          } catch (e2: any) {
            if (connectSeq !== currentConnectSeq) return;
            const s2 = typeof e2 === 'string' ? e2 : (e2?.message || JSON.stringify(e2));
            if (s2.includes('cancelled') || s2.includes('abort') || s2.includes('终止')) {
              set({ status: 'disconnected', connectedNode: null });
              return;
            }
            if (!s2.includes('[中转节点不可用]')) {
              set({ status: 'error', errorMessage: s2 });
              return;
            }
            errStr = s2;
          }
        }
        set({
          status: 'error',
          errorMessage: `已自动尝试 ${queue.length + 1} 个中转节点，均无法连通，没有可用中转节点。\n请在 Servers 列表确认订阅节点状态，或稍后重试。\n最后一次诊断: ${errStr}`,
        });
        return;
      }
      set({ status: 'error', errorMessage: errStr });
    }
  },

  disconnect: async () => {
    currentConnectSeq++;
    set({ status: 'disconnecting', errorMessage: null });
    try {
      await api.disconnect();
      set({ status: 'disconnected', connectedNode: null, connectedChainId: null });
    } catch (e: any) {
      console.error('Disconnect failed:', e);
      set({ status: 'disconnected', errorMessage: `Disconnect failed: ${e}`, connectedChainId: null });
    }
  },

  refreshNodes: async () => {
    try {
      const nodes = await api.getNodes();
      set({ nodes });
    } catch (e: any) {
      console.error('Refresh nodes failed:', e);
    }
  },

  refreshSubscriptions: async () => {
    try {
      const subscriptions = await api.getSubscriptions();
      set({ subscriptions });
    } catch (e: any) {
      console.error('Refresh subscriptions failed:', e);
    }
  },

  applyCustomSubscription: async (customUrl) => {
    const state = get();
    // Wait for subscriptions to be loaded first if empty
    if (state.subscriptions.length === 0) {
       await state.refreshSubscriptions();
    }
    const subscriptions = get().subscriptions;
    
    if (customUrl) {
       // Delete 'Default' if it exists
       const defSub = subscriptions.find(s => s.name === 'Default');
       if (defSub) {
           await get().deleteSubscription(defSub.id);
       }
       
       // Add or update custom sub
       const vipSub = subscriptions.find(s => s.name === '[VIP] 专属订阅');
       if (vipSub) {
          if (vipSub.url !== customUrl) {
             await get().editSubscription(vipSub.id, vipSub.name, customUrl);
             get().updateSubscription(vipSub.id);
          }
       } else {
          await get().addSubscription('[VIP] 专属订阅', customUrl);
       }
    } else {
       // Custom URL is empty, ensure 'Default' exists and VIP is deleted
       const vipSub = subscriptions.find(s => s.name === '[VIP] 专属订阅');
       if (vipSub) {
           await get().deleteSubscription(vipSub.id);
       }
       
       const defSub = subscriptions.find(s => s.name === 'Default');
       if (!defSub) {
          await get().restoreDefaultSubscriptions();
       }
    }
  },

  restoreDefaultSubscriptions: async () => {
    try {
      const subs = await api.restoreDefaultSubscriptions();
      set({ subscriptions: subs });
      // Trigger background update for any new idle subscriptions
      for (const sub of subs) {
        if (!sub.last_updated) {
          get().updateSubscription(sub.id);
        }
      }
    } catch (e: any) {
      set({ errorMessage: `Failed to load default subscriptions: ${e}` });
    }
  },

  updateAllSubscriptions: async () => {
    const subs = get().subscriptions;
    const useProxy = get().updateSubViaProxy;
    set((state) => ({
      subscriptions: state.subscriptions.map((s) => ({ ...s, status: 'updating', error_message: null })),
    }));
    try {
      const updated = await api.updateAllSubscriptions(useProxy);
      set({ subscriptions: updated });
      await get().refreshNodes();
      // 新节点进来,"auto" 中转就该重新量一次
      get().rankRelays();
    } catch (e: any) {
      console.error('Update all subscriptions batch failed, falling back:', e);
      for (const sub of subs) {
        get().updateSubscription(sub.id);
      }
    }
  },

  addSubscription: async (name, url) => {
    try {
      const sub = await api.addSubscription(name, url);
      set((state) => ({ subscriptions: [...state.subscriptions, sub] }));
      // Automatically trigger first update
      get().updateSubscription(sub.id);
      return true;
    } catch (e: any) {
      set({ errorMessage: `Failed to add subscription: ${e}` });
      return false;
    }
  },

  editSubscription: async (id, name, url) => {
    try {
      const updated = await api.editSubscription(id, name, url);
      set((state) => ({
        subscriptions: state.subscriptions.map((s) => (s.id === id ? { ...s, name: updated.name, url: updated.url } : s)),
      }));
      // Refresh nodes to reflect possible group rename
      await get().refreshNodes();
      return true;
    } catch (e: any) {
      set({ errorMessage: `Failed to edit subscription: ${e}` });
      return false;
    }
  },

  deleteSubscription: async (id) => {
    try {
      await api.deleteSubscription(id);
      set((state) => ({
        subscriptions: state.subscriptions.filter((s) => s.id !== id),
      }));
      // Refresh nodes since this subscription's nodes were removed
      await get().refreshNodes();
    } catch (e: any) {
      console.error('Delete subscription failed:', e);
    }
  },

  autoRefreshDefault: async () => {
    const subs = get().subscriptions;
    const defaultSub = subs.find(s => s.name === 'Default');
    if (!defaultSub) return;
    try {
      await get().updateSubscription(defaultSub.id);
      await get().refreshNodes();
      // test all nodes latency in background
      await get().testAllNodes();
      // test all nodes speed in background
      await get().testAllSpeeds();
      // Refresh auth token in background after subscription operations complete
      await get().checkAuth(true).catch(console.error);
    } catch (e) {
      console.error('Failed to auto refresh default sub', e);
    }
  },

  checkAuth: async (forceRemote = false) => {
    try {
      // 1. Get machine ID (retry on Android in case VpnInitProvider hasn't run yet)
      let mId = get().machineId;
      if (!mId) {
        for (let i = 0; i < 3; i++) {
          try {
            mId = await api.getMachineId();
            break;
          } catch (e) {
            if (i < 2) {
              await new Promise(r => setTimeout(r, 1000));
            } else {
              throw e;
            }
          }
        }
        set({ machineId: mId });
      }
      
      // 2. Check Local Cache (if not forced to skip)
      if (!forceRemote) {
        const cachedToken = localStorage.getItem('vpn_auth_token');
        if (cachedToken) {
          const payload = await verifyJWT(cachedToken);
          if (payload && payload.machine_id === mId && payload.authorized) {
            const now = Date.now();
            // Check if CF explicitly expired it
            if (payload.expires_at && now > payload.expires_at) {
              localStorage.removeItem('vpn_auth_token');
              // proceed to remote check
            } else {
              // Check 7-day local cache limit
              const SEVEN_DAYS_MS = 7 * 24 * 3600 * 1000;
              // If token is valid AND has residential_sub_url, use it. Otherwise refresh from remote CF
              if (now - payload.issued_at < SEVEN_DAYS_MS && payload.residential_sub_url) {
                // Locally authorized
                const resUrl = payload.residential_sub_url;
                const effectiveResUrl = (typeof resUrl === 'string' && resUrl.trim().length > 0) ? resUrl.trim() : null;

                set({
                  isAuthorized: true,
                  authDisplayText: payload.display_text,
                  residentialSubUrl: effectiveResUrl,
                });
                if (get().activeTab === 'residential' && !effectiveResUrl) {
                  set({ activeTab: 'dashboard' });
                }
                get().applyCustomSubscription(payload.custom_sub_url || '');
                if (effectiveResUrl) {
                  get().loadResidentialNodes().catch(console.error);
                }
                return true;
              }
            }
          } else {
             // invalid or unauthorized payload
             localStorage.removeItem('vpn_auth_token');
          }
        }
      }

      // 3. Fallback: Request CF via Rust (more reliable than WebView fetch on Android)
      console.log('[checkAuth] Requesting remote auth via Rust, machine_id:', mId);
      let lastError: any = null;
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          const rawBody = await api.requestAuth(mId!);
          const data = JSON.parse(rawBody);
          console.log('[checkAuth] CF Worker response:', JSON.stringify(data));
          
          if (data && data.success && data.authorized && data.token) {
            localStorage.setItem('vpn_auth_token', data.token);
            const payload = await verifyJWT(data.token);
            const resUrl = payload?.residential_sub_url;
            const effectiveResUrl = (typeof resUrl === 'string' && resUrl.trim().length > 0) ? resUrl.trim() : null;

            set({
              isAuthorized: true,
              authDisplayText: data.display_text || null,
              residentialSubUrl: effectiveResUrl,
            });
            if (get().activeTab === 'residential' && !effectiveResUrl) {
              set({ activeTab: 'dashboard' });
            }
            if (payload) get().applyCustomSubscription(payload.custom_sub_url || '');
            if (effectiveResUrl) {
              get().loadResidentialNodes().catch(console.error);
            }
            return true;
          } else {
            console.warn('[checkAuth] Auth failed, response:', JSON.stringify(data));
            localStorage.removeItem('vpn_auth_token');
            const failMsg = data?.display_text || '认证失败';
            set({
              isAuthorized: false,
              authDisplayText: `${failMsg} (ID: ${mId || 'unknown'})`,
              residentialSubUrl: null,
            });
            if (get().activeTab === 'residential') {
              set({ activeTab: 'dashboard' });
            }
            return false;
          }
        } catch (reqErr: any) {
          lastError = reqErr;
          console.warn(`[checkAuth] Auth request attempt ${attempt + 1} failed:`, reqErr?.message || reqErr);
          if (attempt < 2) {
            await new Promise(r => setTimeout(r, 2000 * (attempt + 1)));
          }
        }
      }
      throw new Error(`CF Worker unreachable after 3 attempts: ${lastError?.message || 'unknown error'}`);
    } catch (e: any) {
      console.error('Auth Check Failed', e);
      const errMsg = e?.message || String(e);
      const mId = get().machineId;
      set({
        authDisplayText: `认证异常: ${errMsg} (ID: ${mId || 'pending'})`,
      });
      return get().isAuthorized;
    }
  },

  updateSubscription: async (id) => {
    // Auth Check
    const isAuth = await get().checkAuth();
    if (!isAuth) {
      set({ errorMessage: get().authDisplayText || "认证失败" });
      return;
    }
    
    set((state) => ({
      subscriptions: state.subscriptions.map((s) =>
        s.id === id ? { ...s, status: 'updating', error_message: null } : s
      ),
    }));

    try {
      const updatedSub = await api.updateSubscription(id, get().updateSubViaProxy);
      set((state) => ({
        subscriptions: state.subscriptions.map((s) => (s.id === id ? updatedSub : s)),
      }));
      // Refresh nodes list to reflect newly fetched nodes
      await get().refreshNodes();
      get().rankRelays();
    } catch (e: any) {
      set((state) => ({
        subscriptions: state.subscriptions.map((s) =>
          s.id === id ? { ...s, status: 'error', error_message: String(e) } : s
        ),
      }));
    }
  },

  testLatency: async (id) => {
    set((state) => ({
      testingLatencyIds: [...state.testingLatencyIds, id],
    }));

    try {
      const latency = await api.testNodeLatency(id);
      set((state) => ({
        nodes: state.nodes.map((n) =>
          n.id === id
            ? {
                ...n,
                latency_ms: latency,
                status: latency > 0 ? 'alive' : 'dead',
                last_checked: Date.now() / 1000,
              }
            : n
        ),
      }));
    } catch (e: any) {
      console.error('Test latency failed:', e);
      set((state) => ({
        nodes: state.nodes.map((n) =>
          n.id === id
            ? {
                ...n,
                latency_ms: -1,
                status: 'dead',
                last_checked: Date.now() / 1000,
              }
            : n
        ),
      }));
    } finally {
      set((state) => ({
        testingLatencyIds: state.testingLatencyIds.filter((tid) => tid !== id),
      }));
    }
  },

  testSpeed: async (id) => {
    set((state) => ({
      testingSpeedIds: [...state.testingSpeedIds, id],
    }));

    try {
      const speed = await api.testNodeSpeed(id);
      set((state) => ({
        nodes: state.nodes.map((n) =>
          n.id === id
            ? {
                ...n,
                speed_bps: speed,
                last_checked: Date.now() / 1000,
              }
            : n
        ),
      }));
    } catch (e: any) {
      console.error('Test speed failed:', e);
    } finally {
      set((state) => ({
        testingSpeedIds: state.testingSpeedIds.filter((sid) => sid !== id),
      }));
    }
  },

  testAllNodes: async () => {
    set({ isTestingAll: true });
    try {
      const updatedNodes = await api.testAllNodes();
      set({ nodes: updatedNodes });
    } catch (e: any) {
      console.error('Test all nodes failed:', e);
    } finally {
      set({ isTestingAll: false });
    }
  },

  testAllSpeeds: async () => {
    set({ isTestingAllSpeed: true });
    try {
      const updatedNodes = await api.testAllSpeeds();
      set({ nodes: updatedNodes });
    } catch (e: any) {
      console.error('Test all speeds failed:', e);
    } finally {
      set({ isTestingAllSpeed: false });
    }
  },

  toggleFavorite: async (id) => {
    try {
      const fav = await api.toggleFavorite(id);
      set((state) => ({
        nodes: state.nodes.map((n) => (n.id === id ? { ...n, favorite: fav } : n)),
      }));
    } catch (e: any) {
      console.error('Toggle favorite failed:', e);
    }
  },

  deleteNode: async (id) => {
    try {
      await api.deleteNode(id);
      set((state) => {
        const newNodes = state.nodes.filter((n) => n.id !== id);
        return {
          nodes: newNodes,
          selectedNodeId: state.selectedNodeId === id ? (newNodes[0]?.id || null) : state.selectedNodeId,
        };
      });
    } catch (e: any) {
      console.error('Delete node failed:', e);
    }
  },

  importLink: async (link) => {
    try {
      const node = await api.importShareLink(link);
      set((state) => ({
        nodes: [...state.nodes, node],
        selectedNodeId: node.id,
        activeTab: 'servers',
        errorMessage: null,
  inspectProgress: null,
      }));
      return true;
    } catch (e: any) {
      set({ errorMessage: `Import failed: ${e}` });
      return false;
    }
  },

  saveSettings: async (settings) => {
    try {
      await api.saveSettings(settings);
      set({ settings });
    } catch (e: any) {
      set({ errorMessage: `Failed to save settings: ${e}` });
    }
  },

  refreshChains: async () => {
    try {
      const chains = await api.getChains();
      set({ chains });
    } catch (e: any) {
      console.error('Refresh chains failed:', e);
    }
  },

  addChain: async (chainData) => {
    try {
      const newChain = await api.addChain({
        id: '',
        name: chainData.name,
        remarks: chainData.remarks || '',
        node_ids: chainData.node_ids,
      });
      set((state) => ({ chains: [...state.chains, newChain] }));
      return true;
    } catch (e: any) {
      const errStr = typeof e === 'string' ? e : (e?.message || JSON.stringify(e));
      set({ errorMessage: `创建链式代理失败: ${errStr}` });
      return false;
    }
  },

  updateChain: async (chain) => {
    try {
      await api.updateChain(chain);
      set((state) => ({
        chains: state.chains.map((c) => (c.id === chain.id ? chain : c)),
      }));
      return true;
    } catch (e: any) {
      const errStr = typeof e === 'string' ? e : (e?.message || JSON.stringify(e));
      set({ errorMessage: `更新链式代理失败: ${errStr}` });
      return false;
    }
  },

  deleteChain: async (id) => {
    try {
      await api.deleteChain(id);
      set((state) => ({
        chains: state.chains.filter((c) => c.id !== id),
        connectedChainId: state.connectedChainId === id ? null : state.connectedChainId,
      }));
    } catch (e: any) {
      const errStr = typeof e === 'string' ? e : (e?.message || JSON.stringify(e));
      set({ errorMessage: `删除链式代理失败: ${errStr}` });
    }
  },

  connectChain: async (chainId) => {
    const connectSeq = ++currentConnectSeq;
    const state = get();
    if (state.connectedChainId === chainId && state.status === 'connected') {
      await state.disconnect();
      return;
    }

    set({ status: 'connecting', connectedChainId: chainId, errorMessage: null, tunnelStage: null });
    try {
      await api.connectChain(chainId);
      if (connectSeq !== currentConnectSeq) return;
      const [status, connectedNode] = await Promise.all([
        api.getConnectionStatus(),
        api.getConnectedNode(),
      ]);
      set({ status, connectedNode, connectedChainId: chainId });
    } catch (e: any) {
      if (connectSeq !== currentConnectSeq) return;
      console.error('Chain connect failed:', e);
      const errStr = typeof e === 'string' ? e : (e?.message || JSON.stringify(e));
      if (errStr.includes('cancelled') || errStr.includes('abort') || errStr.includes('终止')) {
        set({ status: 'disconnected', connectedNode: null, connectedChainId: null });
      } else {
        set({
          status: 'error',
          connectedNode: null,
          connectedChainId: null,
          errorMessage: `链式代理连接失败: ${errStr}`,
        });
      }
    }
  },

  connectSmartGroup: async (nodeIds) => {
    const connectSeq = ++currentConnectSeq;
    // Auth Check
    const isAuth = await get().checkAuth();
    if (!isAuth) {
      if (connectSeq === currentConnectSeq) {
        set({ errorMessage: get().authDisplayText || "认证失败" });
      }
      return;
    }

    set({ status: 'connecting', connectedChainId: 'smart-group', errorMessage: null, tunnelStage: null });
    try {
      await api.connectSmartGroup(nodeIds);
      if (connectSeq !== currentConnectSeq) return;
      const [status, connectedNode] = await Promise.all([
        api.getConnectionStatus(),
        api.getConnectedNode(),
      ]);
      set({ status, connectedNode });
    } catch (e: any) {
      if (connectSeq !== currentConnectSeq) return;
      console.error('Smart Group connect failed:', e);
      const errStr = typeof e === 'string' ? e : (e?.message || JSON.stringify(e));
      if (errStr.includes('cancelled') || errStr.includes('abort') || errStr.includes('终止')) {
        set({ status: 'disconnected', connectedNode: null, connectedChainId: null });
      } else {
        set({ status: 'disconnected', errorMessage: `Connect failed: ${e}`, connectedChainId: null });
      }
    }
  },

  testChainLatency: async (chainId) => {
    set((state) => ({
      testingChainIds: [...state.testingChainIds, chainId],
    }));
    try {
      const latency = await api.testChainLatency(chainId);
      set((state) => ({
        chains: state.chains.map((c) => (c.id === chainId ? { ...c, latency_ms: latency } : c)),
        testingChainIds: state.testingChainIds.filter((id) => id !== chainId),
      }));
    } catch (e) {
      set((state) => ({
        chains: state.chains.map((c) => (c.id === chainId ? { ...c, latency_ms: -1 } : c)),
        testingChainIds: state.testingChainIds.filter((id) => id !== chainId),
      }));
    }
  },


  startDeepInspection: async (nodesToTest) => {
    set({ inspectProgress: { current: 0, total: nodesToTest.length, status: "Preparing inspector..." }, inspectReport: null });
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen<{ current: number, total: number, status: string }>('inspector:progress', (event) => {
      set({ inspectProgress: event.payload });
    });
    try {
      const enrichedNodes = await invoke<UnifiedNode[]>('start_deep_inspection', { nodes: nodesToTest });
      
      const items: InspectReportItem[] = [];
      const nodeMap = new Map(nodesToTest.map(n => [n.id, n]));
      
      for (const en of enrichedNodes) {
        const orig = nodeMap.get(en.id);
        if (orig) {
          const latDiff = Math.abs((orig.latency_ms || 0) - (en.latency_ms || 0));
          const spdDiff = Math.abs((orig.speed_bps || 0) - (en.speed_bps || 0));
          const isCountryChanged = orig.country_code !== en.country_code;
          // Filter significant changes
          if (isCountryChanged || latDiff > 50 || spdDiff > 1024 * 1024) {
            items.push({
              id: en.id,
              oldName: orig.name,
              newName: en.name,
              oldCountry: orig.country_code || 'Unknown',
              newCountry: en.country_code,
              oldLatency: orig.latency_ms ?? null,
              newLatency: en.latency_ms ?? null,
              oldSpeed: orig.speed_bps ?? null,
              newSpeed: en.speed_bps ?? null
            });
          }
        }
      }
      
      set({ nodes: get().nodes.map(n => enrichedNodes.find(en => en.id === n.id) || n) });
      for (const node of enrichedNodes) {
        await invoke('update_node', { node });
      }
      
      set({ inspectReport: { items } });
      get().refreshNodes();
    } catch (e: any) {
      set({ errorMessage: `Deep inspection failed: ${e}` });
    } finally {
      unlisten();
      set({ inspectProgress: null });
    }
  },

  testAllChains: async () => {
    const chains = get().chains;
    for (const chain of chains) {
      get().testChainLatency(chain.id);
    }
  },
}));
