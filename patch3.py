import re
with open('src/stores/appStore.ts', 'r', encoding='utf-8') as f:
    content = f.read()

new_cmd = '''
  connectSmartGroup: async (nodeIds) => {
    // Auth Check
    const isAuth = await get().checkAuth();
    if (!isAuth) {
      set({ errorMessage: "请联系服务商授权" });
      return;
    }

    set({ status: 'connecting', connectedChainId: 'smart-group', errorMessage: null });
    try {
      await api.connectSmartGroup(nodeIds);
      const [status, connectedNode] = await Promise.all([
        api.getConnectionStatus(),
        api.getConnectedNode(),
      ]);
      set({ status, connectedNode });
    } catch (e: any) {
      console.error('Smart Group connect failed:', e);
      set({ status: 'disconnected', errorMessage: Connect failed: , connectedChainId: null });
    }
  },
'''

content = re.sub(r'(  connectChain: async.*?\},)\n', r'\1\n' + new_cmd, content, flags=re.DOTALL)

with open('src/stores/appStore.ts', 'w', encoding='utf-8') as f:
    f.write(content)
