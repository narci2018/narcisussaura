import re

with open('src/stores/appStore.ts', 'r', encoding='utf-8') as f:
    content = f.read()

# Add to interface
content = content.replace(
    '  errorMessage: string | null;',
    '  errorMessage: string | null;\n  inspectProgress: { current: number; total: number; status: string } | null;'
)

content = content.replace(
    '  testAllChains: () => Promise<void>;',
    '  testAllChains: () => Promise<void>;\n  startDeepInspection: (nodes: UnifiedNode[]) => Promise<void>;'
)

# Add to initial state
content = content.replace(
    '  errorMessage: null,\n',
    '  errorMessage: null,\n  inspectProgress: null,\n'
)

# Add function implementation
impl = '''
  startDeepInspection: async (nodes) => {
    set({ inspectProgress: { current: 0, total: nodes.length, status: "Preparing inspector..." } });
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen<{ current: number, total: number, status: string }>('inspector:progress', (event) => {
      set({ inspectProgress: event.payload });
    });
    try {
      const enrichedNodes = await api.invoke<UnifiedNode[]>('start_deep_inspection', { nodes });
      set({ nodes: enrichedNodes });
      for (const node of enrichedNodes) {
        await api.invoke('update_node', { node });
      }
      get().refreshNodes();
    } catch (e: any) {
      set({ errorMessage: `Deep inspection failed: ${e}` });
    } finally {
      unlisten();
      setTimeout(() => set({ inspectProgress: null }), 3000);
    }
  },
'''

content = content.replace(
    '  testAllChains: async () => {',
    impl + '\n  testAllChains: async () => {'
)

with open('src/stores/appStore.ts', 'w', encoding='utf-8') as f:
    f.write(content)
