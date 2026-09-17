import re

with open('src/components/SimpleMode/CountryNodeSelector.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

helper = r'''
export function getCountryCodeForNode(node: UnifiedNode): string {
  const upper = node.name.toUpperCase();
  const words = upper.split(/[^A-Z]+/);
  const hasCode = (c: string) => words.includes(c);
  
  let code = '';
  for (const fc of ALL_COUNTRIES) {
    if (
      node.name.includes(fc.zh) || 
      upper.includes(fc.en.toUpperCase()) || 
      hasCode(fc.code)
    ) {
      code = fc.code;
      break;
    }
  }
  
  if (!code) {
    if (node.name.includes('狮城')) code = 'SG';
    else if (node.name.includes('台湾') || node.name.includes('台灣')) code = 'TW';
    else if (upper.includes('TOKYO')) code = 'JP';
    else if (hasCode('USA')) code = 'US';
    else if (hasCode('UK') || upper.includes('BRITAIN')) code = 'GB';
    else if (node.country_code && node.country_code !== '') code = node.country_code;
    else code = 'OTHER';
  }
  return code;
}
'''

# insert helper before CountryGroup
content = content.replace('export interface CountryGroup', helper + '\nexport interface CountryGroup')

# update the internal loop to use getCountryCodeForNode
internal_loop = r'''
      // Force recalculate country code on the frontend to override potentially tainted backend data from older versions
      let code = getCountryCodeForNode(node);
'''

content = re.sub(
    r'      // Force recalculate country code on the frontend.*?else code = \'OTHER\';\n      \}',
    internal_loop.strip(),
    content,
    flags=re.DOTALL
)

with open('src/components/SimpleMode/CountryNodeSelector.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
