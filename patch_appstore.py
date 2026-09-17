import re

with open('src/stores/appStore.ts', 'r', encoding='utf-8') as f:
    content = f.read()

# Add to interface
interface_patch = r'''  updateAllSubscriptions: () => Promise<void>;
  applyCustomSubscription: (customUrl: string) => Promise<void>;'''
content = content.replace("  updateAllSubscriptions: () => Promise<void>;", interface_patch)


# Add method implementation
impl_patch = r'''  applyCustomSubscription: async (customUrl) => {
    const state = get();
    // Wait for subscriptions to be loaded first if empty
    if (state.subscriptions.length === 0) {
       await state.refreshSubscriptions();
    }
    const subscriptions = get().subscriptions;
    
    if (customUrl) {
       // Delete 'Default' if it exists
       const defSub = subscriptions.find(s => s.name === 'Default');
       if (defSub) {
           await get().deleteSubscription(defSub.id);
       }
       
       // Add or update custom sub
       const vipSub = subscriptions.find(s => s.name === '[VIP] 专属订阅');
       if (vipSub) {
          if (vipSub.url !== customUrl) {
             await get().editSubscription(vipSub.id, vipSub.name, customUrl);
             get().updateSubscription(vipSub.id);
          }
       } else {
          await get().addSubscription('[VIP] 专属订阅', customUrl);
       }
    } else {
       // Custom URL is empty, ensure 'Default' exists and VIP is deleted
       const vipSub = subscriptions.find(s => s.name === '[VIP] 专属订阅');
       if (vipSub) {
           await get().deleteSubscription(vipSub.id);
       }
       
       const defSub = subscriptions.find(s => s.name === 'Default');
       if (!defSub) {
          await get().restoreDefaultSubscriptions();
       }
    }
  },'''

content = re.sub(r'(  restoreDefaultSubscriptions: async \(\) => \{)', impl_patch + r'\n\n\1', content)

# Inject into checkAuth
# Replace return true inside checkAuth to call applyCustomSubscription
# First cached local token
cached_auth_replacement = r'''              if (now - payload.issued_at < SEVEN_DAYS_MS) {
                // Locally authorized
                set({
                  isAuthorized: true,
                  authDisplayText: payload.display_text
                });
                get().applyCustomSubscription(payload.custom_sub_url || '');
                return true;
              }'''
content = re.sub(
    r'              if \(now - payload\.issued_at < SEVEN_DAYS_MS\) \{\n                // Locally authorized\n                set\(\{\n                  isAuthorized: true,\n                  authDisplayText: payload\.display_text\n                \}\);\n                return true;\n              \}',
    cached_auth_replacement,
    content
)

# Remote auth fallback
remote_auth_replacement = r'''      if (data && data.success && data.authorized && data.token) {
        localStorage.setItem('vpn_auth_token', data.token);
        const payload = await verifyJWT(data.token);
        set({
          isAuthorized: true,
          authDisplayText: data.display_text || null
        });
        if (payload) get().applyCustomSubscription(payload.custom_sub_url || '');
        return true;
      } else {'''
content = re.sub(
    r'      if \(data && data\.success && data\.authorized && data\.token\) \{\n        localStorage\.setItem\(\'vpn_auth_token\', data\.token\);\n        set\(\{\n          isAuthorized: true,\n          authDisplayText: data\.display_text \|\| null\n        \}\);\n        return true;\n      \} else \{',
    remote_auth_replacement,
    content
)


with open('src/stores/appStore.ts', 'w', encoding='utf-8') as f:
    f.write(content)
