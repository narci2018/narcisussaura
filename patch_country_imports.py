import re

with open('src/components/SimpleMode/CountryNodeSelector.tsx', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('useState, useMemo, useState', 'useMemo, useState')
content = content.replace('ListFilter } from', 'ListFilter, Search } from')

with open('src/components/SimpleMode/CountryNodeSelector.tsx', 'w', encoding='utf-8') as f:
    f.write(content)
