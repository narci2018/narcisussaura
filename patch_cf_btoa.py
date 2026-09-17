import re

with open('server/src/index.js', 'r', encoding='utf-8') as f:
    content = f.read()

# Fix btoaUrl
btoa_patch = r'''function btoaUrl(str) {
  return btoa(unescape(encodeURIComponent(str))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}'''
content = re.sub(
    r'function btoaUrl\(str\) \{\n  return btoa\(str\)\.replace\(\/\\\+\/g, \'-\'\)\.replace\(\/\\\/\/g, \'_\'\)\.replace\(\/=\/g, \'\'\);\n\}',
    btoa_patch,
    content
)

with open('server/src/index.js', 'w', encoding='utf-8') as f:
    f.write(content)
