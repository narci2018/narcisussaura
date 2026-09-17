import re

with open('server/src/index.js', 'r', encoding='utf-8') as f:
    content = f.read()

# Revert btoaUrl to normal binary base64
btoa_patch = r'''function btoaUrl(str) {
  return btoa(str).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}'''
content = re.sub(
    r'function btoaUrl\(str\) \{\n  return btoa\(unescape\(encodeURIComponent\(str\)\)\)\.replace\(\/\\\+\/g, \'-\'\)\.replace\(\/\\\/\/g, \'_\'\)\.replace\(\/=\/g, \'\'\);\n\}',
    btoa_patch,
    content
)

# Fix signJWT to encode UTF-8 correctly for header and body before btoaUrl
sign_patch = r'''async function signJWT(payload) {
  const enc = new TextEncoder();
  
  // 对于可能包含非拉丁字符的内容，先进行 UTF-8 编码为 binary string
  const encodeUtf8 = (s) => unescape(encodeURIComponent(s));
  
  const header = btoaUrl(encodeUtf8(JSON.stringify({ alg: "HS256", typ: "JWT" })));
  const body = btoaUrl(encodeUtf8(JSON.stringify(payload)));
  const data = ${header}.;
  
  const key = await crypto.subtle.importKey(
    "raw", enc.encode(JWT_SECRET),
    { name: "HMAC", hash: "SHA-256" },
    false, ["sign"]
  );
  
  const signature = await crypto.subtle.sign("HMAC", key, enc.encode(data));
  // signature 已经是 raw bytes，直接转换
  const sigBase64 = btoaUrl(String.fromCharCode(...new Uint8Array(signature)));
  
  return ${data}.;
}'''

content = re.sub(
    r'async function signJWT\(payload\) \{.*?\n\}',
    sign_patch,
    content,
    flags=re.DOTALL
)

with open('server/src/index.js', 'w', encoding='utf-8') as f:
    f.write(content)
