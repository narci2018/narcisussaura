const fs = require('fs');
let lines = fs.readFileSync('src/components/ServerList.tsx', 'utf8').split('\n');
lines[302] = "                          {item.oldLatency ? ${item.oldLatency}ms : '无'}";
lines[312] = "                          {item.oldSpeed ? ${(item.oldSpeed / (1024 * 1024)).toFixed(1)}M : '无'}";
lines[316] = "                          {item.newSpeed ? ${(item.newSpeed / (1024 * 1024)).toFixed(1)}M : '0M'}";
fs.writeFileSync('src/components/ServerList.tsx', lines.join('\n'));

let lines2 = fs.readFileSync('src/stores/appStore.ts', 'utf8').split('\n');
for (let i = 0; i < lines2.length; i++) {
  if (lines2[i].includes('errorMessage: Deep inspection failed:')) {
    lines2[i] = "      set({ errorMessage: Deep inspection failed:  });";
  }
}
fs.writeFileSync('src/stores/appStore.ts', lines2.join('\n'));
