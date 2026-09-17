import re

with open('src/components/SimpleMode/SimpleDashboard.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

# Update resolvedNodeId
resolved_replacement = r'''  const resolvedNodeId = useMemo(() => {
    if (selectedValue === 'smart') return globalBest?.id ?? null;
    if (selectedValue.startsWith('country:')) {
      const code = selectedValue.substring(8);
      const nodesInCountry = regularNodes.filter(n => {
        const upper = n.name.toUpperCase();
        return upper.includes(code) || n.country_code === code || n.name.includes(code);
      });
      return getBestNode(nodesInCountry)?.id ?? null;
    }
    return selectedValue;
  }, [selectedValue, globalBest, regularNodes]);'''

content = re.sub(r'  const resolvedNodeId = useMemo\(\(\) => \{.*?\}, \[selectedValue, globalBest\]\);', resolved_replacement, content, flags=re.DOTALL)


# Update handleToggle
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
        
        // Find nodes matching this country code
        // We reuse the logic that CountryNodeSelector uses, or simply rely on the regularNodes and search.
        // But to be 100% consistent, we can just grab them by country code.
        const countryNodes = [...regularNodes].filter(n => {
           // We need a robust check. In CountryNodeSelector it's built into countryGroups. 
           // We can just filter them here simply by code or name match for now, or we can export a helper.
           // Let's do a simple check.
           const codeUpper = code.toUpperCase();
           if (n.country_code === codeUpper) return true;
           // basic fallback
           const upper = n.name.toUpperCase();
           const words = upper.split(/[^A-Z]+/);
           if (words.includes(codeUpper)) return true;
           return false;
        });
        
        const topNodes = countryNodes
          .sort((a, b) => (a.latency_ms || 9999) - (b.latency_ms || 9999))
          .slice(0, 10)
          .map(n => n.id);
          
        if (topNodes.length > 0) {
          useAppStore.getState().connectSmartGroup(topNodes);
        } else {
           // fallback if none found
           if (!resolvedNodeId) return;
           connect(resolvedNodeId ?? undefined);
        }
      } else {
        if (!resolvedNodeId) return;
        connect(resolvedNodeId ?? undefined);
      }'''

content = re.sub(r'      if \(selectedValue === \'smart\'\) \{.*?\n      \} else \{\n        if \(\!resolvedNodeId\) return;\n        connect\(resolvedNodeId \?\? undefined\);\n      \}', handle_toggle_replacement, content, flags=re.DOTALL)


# Update displayNodeName
display_node_replacement = r'''  const displayNodeName = isConnected
    ? (connectedChainId === 'smart-group' ? ${connectedNode?.name} (智能漂移) : connectedNode?.name)
    : selectedValue === 'smart'
    ? globalBest?.name
    : selectedValue.startsWith('country:')
    ? (resolvedNodeId ? nodes.find(n => n.id === resolvedNodeId)?.name : '未知节点')
    : nodes.find((n) => n.id === selectedValue)?.name;'''

content = re.sub(r'  const displayNodeName = isConnected.*?: nodes\.find\(\(n\) => n\.id === selectedValue\)\?\.name;', display_node_replacement, content, flags=re.DOTALL)

# Update displayCountry
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
