import re

with open('src/stores/appStore.ts', 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace('set({ errorMessage: Deep inspection failed:  });', 'set({ errorMessage: Deep inspection failed:  });')
content = content.replace('},,', '},')

with open('src/stores/appStore.ts', 'w', encoding='utf-8') as f:
    f.write(content)
