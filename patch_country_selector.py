import re

with open('src/components/SimpleMode/CountryNodeSelector.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

# Add useState import if not exists
# it probably exists, but let's check
if 'useState,' not in content:
    content = content.replace('import React, { useMemo', 'import React, { useState, useMemo')
elif 'useState' not in content:
    content = content.replace('import React, { ', 'import React, { useState, ')

# Add filterText state
state_code = '''
  const [open, setOpen] = useState(false);
  const [filterText, setFilterText] = useState('');
'''
content = content.replace('  const [open, setOpen] = useState(false);', state_code)

# Filter countryGroups
filter_code = '''
  // Filter country groups
  const filteredCountryGroups = useMemo(() => {
    if (!filterText) return countryGroups;
    const lower = filterText.toLowerCase();
    return countryGroups.filter(cg => 
      cg.countryZh.toLowerCase().includes(lower) || 
      cg.country.toLowerCase().includes(lower)
    );
  }, [countryGroups, filterText]);
'''
content = content.replace('  // Find the currently selected country group', filter_code + '\n  // Find the currently selected country group')

# Add Search input UI
input_code = '''
          {/* Search Input */}
          <div className="px-3 py-2 border-b border-[#1e2535]">
            <div className="relative">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-500" />
              <input
                type="text"
                value={filterText}
                onChange={e => setFilterText(e.target.value)}
                placeholder="搜索国家或地区..."
                className="w-full bg-[#12151f] border border-[#2a3050] text-sm text-gray-200 placeholder-gray-600 rounded-lg pl-8 pr-3 py-1.5 focus:outline-none focus:border-indigo-500/50"
                onClick={(e) => e.stopPropagation()}
              />
            </div>
          </div>
'''
content = content.replace(
    '          {/* Country list */}\n          <div className="overflow-y-auto">',
    input_code + '\n          {/* Country list */}\n          <div className="overflow-y-auto">'
)

# Render filteredCountryGroups instead of countryGroups
content = content.replace('            {countryGroups.map((cg) => {', '            {filteredCountryGroups.map((cg) => {')

# Import Search icon if needed
if 'Search' not in content:
    content = content.replace('AlertCircle,', 'AlertCircle, Search,')

with open('src/components/SimpleMode/CountryNodeSelector.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
