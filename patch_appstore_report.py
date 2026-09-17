import re

with open('src/stores/appStore.ts', 'r', encoding='utf-8') as f:
    content = f.read()

# Add interface InspectReport
report_interface = '''
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
'''
if 'export interface InspectReportItem' not in content:
    content = content.replace('interface AppStore {', report_interface + '\ninterface AppStore {')

# Add to AppStore
if 'inspectReport: InspectReport | null;' not in content:
    content = content.replace('  inspectProgress: { current: number; total: number; status: string } | null;', '  inspectProgress: { current: number; total: number; status: string } | null;\n  inspectReport: InspectReport | null;\n  closeInspectReport: () => void;')

# Add to state initialization
if 'inspectReport: null,' not in content:
    content = content.replace('inspectProgress: null,', 'inspectProgress: null,\n  inspectReport: null,')

# Add close action
if 'closeInspectReport: ()' not in content:
    content = content.replace('setErrorMessage: (errorMessage) => set({ errorMessage }),', 'setErrorMessage: (errorMessage) => set({ errorMessage }),\n  closeInspectReport: () => set({ inspectReport: null }),')

# Modify startDeepInspection
replacement = '''
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
      set({ errorMessage: Deep inspection failed:  });
    } finally {
      unlisten();
      set({ inspectProgress: null });
    }
  },
'''
content = re.sub(r'startDeepInspection: async \(nodes\) => \{.*?\n  \},', replacement.strip() + ',', content, flags=re.DOTALL)

with open('src/stores/appStore.ts', 'w', encoding='utf-8') as f:
    f.write(content)
