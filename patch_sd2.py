import re

with open('src/components/SimpleMode/SimpleDashboard.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

# import it
content = content.replace("import { CountryNodeSelector, getBestNode } from './CountryNodeSelector';", "import { CountryNodeSelector, getBestNode, getCountryCodeForNode } from './CountryNodeSelector';")

# fix resolvedNodeId
resolved_replacement = r'''  const resolvedNodeId = useMemo(() => {
    if (selectedValue === 'smart') return globalBest?.id ?? null;
    if (selectedValue.startsWith('country:')) {
      const code = selectedValue.substring(8);
      const nodesInCountry = regularNodes.filter(n => getCountryCodeForNode(n) === code);
      return getBestNode(nodesInCountry)?.id ?? null;
    }
    return selectedValue;
  }, [selectedValue, globalBest, regularNodes]);'''

content = re.sub(r'  const resolvedNodeId = useMemo\(\(\) => \{.*?\}, \[selectedValue, globalBest, regularNodes\]\);', resolved_replacement, content, flags=re.DOTALL)

# fix handleToggle
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

content = re.sub(r'      if \(selectedValue === \'smart\'\) \{.*?\n      \} else if \(selectedValue\.startsWith\(\'country:\'\)\) \{.*?\n      \} else \{\n        if \(\!resolvedNodeId\) return;\n        connect\(resolvedNodeId \?\? undefined\);\n      \}', handle_toggle_replacement, content, flags=re.DOTALL)

with open('src/components/SimpleMode/SimpleDashboard.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
