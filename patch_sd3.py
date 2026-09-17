import re

with open('src/components/SimpleMode/SimpleDashboard.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace("import { CountryNodeSelector, getBestNode } from './CountryNodeSelector';", "import { CountryNodeSelector, getBestNode, getCountryCodeForNode } from './CountryNodeSelector';")

resolved_replacement = r'''  const resolvedNodeId = useMemo(() => {
    if (selectedValue === 'smart') return globalBest?.id ?? null;
    if (selectedValue.startsWith('country:')) {
      const code = selectedValue.substring(8);
      const nodesInCountry = regularNodes.filter(n => getCountryCodeForNode(n) === code);
      return getBestNode(nodesInCountry)?.id ?? null;
    }
    return selectedValue;
  }, [selectedValue, globalBest, regularNodes]);'''

content = re.sub(r'  const resolvedNodeId = useMemo\(\(\) => \{.*?\}, \[selectedValue, globalBest\]\);', resolved_replacement, content, flags=re.DOTALL)

handle_toggle_replacement = r'''      if (selectedValue === 'smart') {
        const topNodes = [...regularNodes]
          .sort((a, b) => (a.latency_ms || 9999) - (b.latency_ms || 9999))
          .slice(0, 10)
          .map(n => n.id);
        if (topNodes.length > 0) {
          useAppStore.getState().connectSmartGroup(topNodes);
        }
      } else if (selectedValue.startsWith('country:')) {
        const code = selectedValue.substring(8);
        const countryNodes = [...regularNodes].filter(n => getCountryCodeForNode(n) === code);
        
        const topNodes = countryNodes
          .sort((a, b) => (a.latency_ms || 9999) - (b.latency_ms || 9999))
          .slice(0, 10)
          .map(n => n.id);
          
        if (topNodes.length > 0) {
          useAppStore.getState().connectSmartGroup(topNodes);
        } else {
           if (!resolvedNodeId) return;
           connect(resolvedNodeId ?? undefined);
        }
      } else {
        if (!resolvedNodeId) return;
        connect(resolvedNodeId ?? undefined);
      }'''

content = re.sub(r'      if \(\!resolvedNodeId\) return;\n      connect\(resolvedNodeId \?\? undefined\);', handle_toggle_replacement, content, count=1)

display_node_replacement = r'''  const displayNodeName = isConnected
    ? (connectedChainId === 'smart-group' ? `${connectedNode?.name} (智能漂移)` : connectedNode?.name)
    : selectedValue === 'smart'
    ? globalBest?.name
    : selectedValue.startsWith('country:')
    ? (resolvedNodeId ? nodes.find(n => n.id === resolvedNodeId)?.name : '未知节点')
    : nodes.find((n) => n.id === selectedValue)?.name;'''

content = re.sub(r'  const displayNodeName = isConnected.*?: nodes\.find\(\(n\) => n\.id === selectedValue\)\?\.name;', display_node_replacement, content, flags=re.DOTALL)

display_country_replacement = r'''  const displayCountry = isConnected
    ? connectedNode?.country_name
    : selectedValue === 'smart'
    ? globalBest?.country_name
    : selectedValue.startsWith('country:')
    ? (resolvedNodeId ? nodes.find(n => n.id === resolvedNodeId)?.country_name : '未知')
    : nodes.find((n) => n.id === selectedValue)?.country_name;'''

content = re.sub(r'  const displayCountry = isConnected.*?: nodes\.find\(\(n\) => n\.id === selectedValue\)\?\.country_name;', display_country_replacement, content, flags=re.DOTALL)

with open('src/components/SimpleMode/SimpleDashboard.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
