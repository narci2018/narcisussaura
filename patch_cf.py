import re

with open('server/src/index.js', 'r', encoding='utf-8') as f:
    content = f.read()

# 1. Update HTML template to add Custom Subscription URL input
custom_sub_input = r'''                <div class="mb-4">
                    <label class="block text-sm font-medium text-gray-700 mb-1">自定义显示文本 (App 左上角)</label>
                    <input v-model="authForm.display_text" type="text" class="w-full px-3 py-2 border rounded focus:outline-none focus:ring-1 focus:ring-indigo-500">
                </div>
                
                <div class="mb-4">
                    <label class="block text-sm font-medium text-gray-700 mb-1">专属订阅源 (留空则用Default)</label>
                    <input v-model="authForm.custom_sub_url" type="text" class="w-full px-3 py-2 border rounded focus:outline-none focus:ring-1 focus:ring-indigo-500" placeholder="https://...">
                </div>'''

content = re.sub(
    r'                <div class="mb-4">\s*<label class="block text-sm font-medium text-gray-700 mb-1">自定义显示文本 \(App 左上角\)</label>\s*<input v-model="authForm\.display_text" type="text" class="w-full px-3 py-2 border rounded focus:outline-none focus:ring-1 focus:ring-indigo-500">\s*</div>',
    custom_sub_input,
    content
)

# 2. Update Vue state: authForm = ref({ display_text: '', days: '', custom_sub_url: '' });
content = content.replace(
    "const authForm = ref({ display_text: '', days: '' });",
    "const authForm = ref({ display_text: '', days: '', custom_sub_url: '' });"
)

# 3. Update openAuthModal
open_auth_modal_replacement = r'''                const openAuthModal = (device) => {
                    currentDevice.value = device;
                    authForm.value.display_text = device.display_text || 'NarcissusAura VIP';
                    authForm.value.custom_sub_url = device.custom_sub_url || '';'''
content = content.replace(
    '''                const openAuthModal = (device) => {
                    currentDevice.value = device;
                    authForm.value.display_text = device.display_text || 'NarcissusAura VIP';''',
    open_auth_modal_replacement
)

# 4. Update submitAuth
submit_auth_replacement = r'''                        body: JSON.stringify({
                            machine_id: currentDevice.value.machine_id,
                            authorized: true,
                            display_text: authForm.value.display_text,
                            custom_sub_url: authForm.value.custom_sub_url,
                            days: authForm.value.days === '' ? 0 : Number(authForm.value.days)
                        })'''
content = re.sub(
    r'''                        body: JSON\.stringify\(\{
                            machine_id: currentDevice\.value\.machine_id,
                            authorized: true,
                            display_text: authForm\.value\.display_text,
                            days: authForm\.value\.days === '' \? 0 : Number\(authForm\.value\.days\)
                        \}\)''',
    submit_auth_replacement,
    content
)

# 5. Update revokeAuth (just for safety)
revoke_auth_replacement = r'''                        body: JSON.stringify({
                            machine_id: currentDevice.value.machine_id,
                            authorized: false,
                            display_text: '未授权设备',
                            custom_sub_url: '',
                            days: 0
                        })'''
content = re.sub(
    r'''                        body: JSON\.stringify\(\{
                            machine_id: currentDevice\.value\.machine_id,
                            authorized: false,
                            display_text: '未授权设备',
                            days: 0
                        \}\)''',
    revoke_auth_replacement,
    content
)

# 6. Backend /api/admin/update logic
# It needs to extract custom_sub_url and save it. Let's find it.
'''
      } else if (url.pathname === '/api/admin/update') {
        const body = await request.json();
        const { machine_id, authorized, display_text, days } = body;
...
        const updatedData = {
          ...oldData,
          authorized,
          display_text,
          expires_at: expiresAt,
          last_seen: Date.now()
        };
'''
admin_update_backend_replacement = r'''      } else if (url.pathname === '/api/admin/update') {
        const body = await request.json();
        const { machine_id, authorized, display_text, days, custom_sub_url } = body;
        if (!machine_id) return Response.json({ success: false });

        let oldData = {};
        const oldStr = await env.AUTH_KV.get(machine_id);
        if (oldStr) {
          try { oldData = JSON.parse(oldStr); } catch(e){}
        }

        let expiresAt = null;
        let ttlConfig = {};
        if (authorized && days > 0) {
           expiresAt = Date.now() + (days * 86400 * 1000);
           ttlConfig = { expirationTtl: days * 86400 }; 
        }

        const updatedData = {
          ...oldData,
          authorized,
          display_text,
          custom_sub_url,
          expires_at: expiresAt,
          last_seen: Date.now()
        };'''
content = re.sub(
    r'''      \} else if \(url\.pathname === '/api/admin/update'\) \{
        const body = await request\.json\(\);
        const \{ machine_id, authorized, display_text, days \} = body;.*?const updatedData = \{
          \.\.\.oldData,
          authorized,
          display_text,
          expires_at: expiresAt,
          last_seen: Date\.now\(\)
        \};''',
    admin_update_backend_replacement,
    content,
    flags=re.DOTALL
)

# 7. Backend /api/auth JWT payload
'''
        // --- JWT 签名颁发 ---
        const payload = {
          machine_id: machineId,
          authorized: true,
          display_text: userData.display_text || "NarcissusAura VIP",
          expires_at: userData.expires_at || null, // CF 中的最终到期时间
          issued_at: Date.now() // 发证时间，App 用它判断 7 天缓存期
        };
'''
jwt_payload_replacement = r'''        // --- JWT 签名颁发 ---
        const payload = {
          machine_id: machineId,
          authorized: true,
          display_text: userData.display_text || "NarcissusAura VIP",
          custom_sub_url: userData.custom_sub_url || "",
          expires_at: userData.expires_at || null, // CF 中的最终到期时间
          issued_at: Date.now() // 发证时间，App 用它判断 7 天缓存期
        };'''
content = re.sub(
    r'''        // --- JWT 签名颁发 ---\s*const payload = \{\s*machine_id: machineId,\s*authorized: true,\s*display_text: userData\.display_text \|\| "NarcissusAura VIP",\s*expires_at: userData\.expires_at \|\| null, // CF 中的最终到期时间\s*issued_at: Date\.now\(\) // 发证时间，App 用它判断 7 天缓存期\s*\};''',
    jwt_payload_replacement,
    content
)

with open('server/src/index.js', 'w', encoding='utf-8') as f:
    f.write(content)
