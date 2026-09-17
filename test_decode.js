const str = '{"text":"Master尊享"}';
const btoaUrl = (s) => Buffer.from(unescape(encodeURIComponent(s)), 'binary').toString('base64').replace(/\+/g, '-',).replace(/\//g, '_').replace(/=/g, '');
const encoded = btoaUrl(str);

let bodyBase64 = encoded.replace(/-/g, '+').replace(/_/g, '/');
while (bodyBase64.length % 4) bodyBase64 += '=';
const decoded = JSON.parse(decodeURIComponent(escape(Buffer.from(bodyBase64, 'base64').toString('binary'))));
console.log(decoded);
