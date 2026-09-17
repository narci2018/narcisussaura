import re

with open('src/components/SimpleMode/CountryNodeSelector.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

# Replace handleCountryClick
content = re.sub(
    r'  const handleCountryClick = \(cg: CountryGroup\) => \{\n    if \(!cg\.hasAlive\) return;\n    if \(cg\.bestNode\) \{\n      onChange\(cg\.bestNode\.id\);\n      setOpen\(false\);\n    \}\n  \};',
    r'''  const handleCountryClick = (cg: CountryGroup) => {
    if (!cg.hasAlive) return;
    onChange(country:);
    setOpen(false);
  };''',
    content
)

# Update currentLabel
current_label_replacement = r'''  const currentLabel = useMemo(() => {
    if (value === 'smart') {
      return { primary: '🌐 智能最优线路', secondary: globalBest ? 自动选择:  : '无可用节点' };
    }
    if (value.startsWith('country:')) {
      const code = value.substring(8);
      const cg = countryGroups.find(c => c.country === code);
      if (cg && cg.bestNode) {
        return { primary: cg.countryZh, secondary: 智能漂移 (自动分配) };
      }
    }
    // Find which country+node
    for (const cg of countryGroups) {
      const found = cg.nodes.find((n) => n.id === value);
      if (found) {
        return { primary: cg.countryZh, secondary: found.name };
      }
    }
    return { primary: '请选择节点', secondary: '' };
  }, [value, globalBest, countryGroups]);'''

content = re.sub(r'  const currentLabel = useMemo\(\(\) => \{.*?\}, \[value, globalBest, countryGroups\]\);', current_label_replacement, content, flags=re.DOTALL)

# Update selectedSpeed and selectedLatency
speed_replacement = r'''  const selectedSpeed = useMemo(() => {
    if (value === 'smart') return globalBest?.speed_bps;
    if (value.startsWith('country:')) {
      const code = value.substring(8);
      const cg = countryGroups.find(c => c.country === code);
      return cg?.bestNode?.speed_bps;
    }
    return nodes.find((n) => n.id === value)?.speed_bps;
  }, [value, globalBest, nodes, countryGroups]);'''

latency_replacement = r'''  const selectedLatency = useMemo(() => {
    if (value === 'smart') return globalBest?.latency_ms;
    if (value.startsWith('country:')) {
      const code = value.substring(8);
      const cg = countryGroups.find(c => c.country === code);
      return cg?.bestNode?.latency_ms;
    }
    return nodes.find((n) => n.id === value)?.latency_ms;
  }, [value, globalBest, nodes, countryGroups]);'''

content = re.sub(r'  const selectedSpeed = useMemo\(\(\) => \{.*?\}, \[value, globalBest, nodes\]\);', speed_replacement, content, flags=re.DOTALL)
content = re.sub(r'  const selectedLatency = useMemo\(\(\) => \{.*?\}, \[value, globalBest, nodes\]\);', latency_replacement, content, flags=re.DOTALL)

# Update currentCountryGroup
ccg_replacement = r'''  // Find the currently selected country group (for manual picker button)
  const currentCountryGroup = useMemo(() => {
    if (value === 'smart') return null;
    if (value.startsWith('country:')) {
      const code = value.substring(8);
      return countryGroups.find((cg) => cg.country === code) ?? null;
    }
    return countryGroups.find((cg) => cg.nodes.some((n) => n.id === value)) ?? null;
  }, [value, countryGroups]);'''
content = re.sub(r'  // Find the currently selected country group \(for manual picker button\)\n  const currentCountryGroup = useMemo\(\(\) => \{.*?\}, \[value, countryGroups\]\);', ccg_replacement, content, flags=re.DOTALL)

# Update dropdown highlighting
content = content.replace(
    'const isSelected = cg.nodes.some((n) => n.id === value);',
    "const isSelected = value === country: || (!value.startsWith('country:') && cg.nodes.some((n) => n.id === value));"
)

with open('src/components/SimpleMode/CountryNodeSelector.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
