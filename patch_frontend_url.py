import re

with open('src/stores/appStore.ts', 'r', encoding='utf-8') as f:
    content = f.read()

# Replace CF_AUTH_URL
content = re.sub(
    r'const CF_AUTH_URL = "https://vpn-auth-server\.narci-ltc\.workers\.dev/api/auth";',
    r'const CF_AUTH_URL = "https://auth.lkhotrich.kdns.fr/api/auth";',
    content
)

with open('src/stores/appStore.ts', 'w', encoding='utf-8') as f:
    f.write(content)
