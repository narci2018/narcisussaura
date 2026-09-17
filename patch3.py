import re
with open('src/components/ServerList.tsx', 'r', encoding='utf-8') as f:
    text = f.read()

text = text.replace('{item.oldLatency ? ms', '{item.oldLatency ? ${item.oldLatency}ms')
text = text.replace('{item.oldSpeed ? M', '{item.oldSpeed ? ${(item.oldSpeed / (1024 * 1024)).toFixed(1)}M')
text = text.replace('{item.newSpeed ? M', '{item.newSpeed ? ${(item.newSpeed / (1024 * 1024)).toFixed(1)}M')
# Oh, the colon part is missing a backtick in my first attempt... wait!
# If it is {item.oldLatency ? ms : '无'}
# and I replace '{item.oldLatency ? ms' with '{item.oldLatency ? ${item.oldLatency}ms'
# then the rest remains: : '无'} -> so it becomes '{item.oldLatency ? ${item.oldLatency}ms : '无'}' which is CORRECT!

with open('src/components/ServerList.tsx', 'w', encoding='utf-8') as f:
    f.write(text)

with open('src/stores/appStore.ts', 'r', encoding='utf-8') as f:
    text2 = f.read()
text2 = text2.replace('set({ errorMessage: Deep inspection failed:  });', 'set({ errorMessage: Deep inspection failed:  });')
with open('src/stores/appStore.ts', 'w', encoding='utf-8') as f:
    f.write(text2)
