const ADMIN_PASSWORD = "admin"; // 管理员密码
const JWT_SECRET = "NARCISSUS_AURA_SUPER_SECRET_KEY_2026"; // 签名密钥

// Base64URL 编码辅助函数
function btoaUrl(str) {
  return btoa(str).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}

// 颁发数字签名证书 (JWT)
async function signJWT(payload) {
  const enc = new TextEncoder();
  const header = btoaUrl(JSON.stringify({ alg: "HS256", typ: "JWT" }));
  const body = btoaUrl(JSON.stringify(payload));
  const data = `${header}.${body}`;
  
  const key = await crypto.subtle.importKey(
    "raw", enc.encode(JWT_SECRET),
    { name: "HMAC", hash: "SHA-256" },
    false, ["sign"]
  );
  
  const signature = await crypto.subtle.sign("HMAC", key, enc.encode(data));
  const sigBase64 = btoaUrl(String.fromCharCode(...new Uint8Array(signature)));
  
  return `${data}.${sigBase64}`;
}

const ADMIN_HTML = `
<!DOCTYPE html>
<html lang="zh">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>VPN 授权管理后台</title>
    <script src="https://unpkg.com/vue@3/dist/vue.global.js"></script>
    <script src="https://cdn.tailwindcss.com"></script>
    <style>
        [v-cloak] { display: none; }
    </style>
</head>
<body class="bg-gray-100 text-gray-800">
    <div id="app" v-cloak class="min-h-screen">
        <!-- 登录页 -->
        <div v-if="!isLoggedIn" class="flex items-center justify-center min-h-screen bg-gray-200">
            <div class="p-8 bg-white rounded-lg shadow-md w-96">
                <h1 class="text-2xl font-bold mb-6 text-center text-indigo-600">管理后台登录</h1>
                <input v-model="password" type="password" placeholder="请输入管理员密码" class="w-full px-4 py-2 border rounded mb-4 focus:outline-none focus:ring-2 focus:ring-indigo-400" @keyup.enter="login">
                <button @click="login" class="w-full bg-indigo-600 text-white font-bold py-2 rounded hover:bg-indigo-700 transition">登录</button>
            </div>
        </div>

        <!-- 主面板 -->
        <div v-else class="container mx-auto p-6 max-w-7xl">
            <div class="flex justify-between items-center mb-6">
                <h1 class="text-3xl font-extrabold text-gray-800 flex items-center gap-2">
                    🛡️ VPN 授权控制台
                </h1>
                <button @click="logout" class="text-sm text-gray-500 hover:text-red-500 underline">退出登录</button>
            </div>

            <!-- 数据表格 -->
            <div class="bg-white rounded-xl shadow overflow-x-auto">
                <table class="min-w-full divide-y divide-gray-200">
                    <thead class="bg-gray-50">
                        <tr>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">机器码</th>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">状态</th>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">显示文本</th>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">到期时间</th>
                            <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">最后活跃</th>
                            <th class="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">操作</th>
                        </tr>
                    </thead>
                    <tbody class="bg-white divide-y divide-gray-200">
                        <tr v-for="device in devices" :key="device.machine_id">
                            <td class="px-6 py-4 whitespace-nowrap text-sm font-mono text-gray-600">{{ device.machine_id }}</td>
                            <td class="px-6 py-4 whitespace-nowrap">
                                <span v-if="device.authorized" class="px-2 inline-flex text-xs leading-5 font-semibold rounded-full bg-green-100 text-green-800">已授权</span>
                                <span v-else class="px-2 inline-flex text-xs leading-5 font-semibold rounded-full bg-yellow-100 text-yellow-800">待审批</span>
                            </td>
                            <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-900">{{ device.display_text || '-' }}</td>
                            <td class="px-6 py-4 whitespace-nowrap text-sm font-medium" :class="getExpiryClass(device.expires_at)">
                                {{ formatExpires(device.expires_at, device.authorized) }}
                            </td>
                            <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">{{ formatDate(device.last_seen) }}</td>
                            <td class="px-6 py-4 whitespace-nowrap text-right text-sm font-medium">
                                <button @click="openAuthModal(device)" class="text-indigo-600 hover:text-indigo-900 mr-3">设置</button>
                                <button @click="deleteDevice(device.machine_id)" class="text-red-600 hover:text-red-900">删除</button>
                            </td>
                        </tr>
                        <tr v-if="devices.length === 0">
                            <td colspan="6" class="px-6 py-10 text-center text-gray-500">暂无任何设备记录</td>
                        </tr>
                    </tbody>
                </table>
            </div>
            
            <div class="mt-4 flex justify-between items-center text-sm text-gray-500">
                <span>提示：已启用数字签名。App 本地缓存最长保留 7 天，到期自动向服务器重新换证。</span>
                <button @click="fetchDevices" class="flex items-center gap-1 hover:text-indigo-600">刷新数据</button>
            </div>
        </div>

        <!-- 授权弹窗 -->
        <div v-if="showModal" class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center">
            <div class="bg-white p-6 rounded-lg shadow-xl w-96">
                <h2 class="text-xl font-bold mb-4">设备授权配置</h2>
                <div class="mb-4 font-mono text-xs text-gray-500 break-all">{{ currentDevice?.machine_id }}</div>
                
                <div class="mb-4">
                    <label class="block text-sm font-medium text-gray-700 mb-1">自定义显示文本 (App 左上角)</label>
                    <input v-model="authForm.display_text" type="text" class="w-full px-3 py-2 border rounded focus:outline-none focus:ring-1 focus:ring-indigo-500">
                </div>
                
                <div class="mb-4">
                    <label class="block text-sm font-medium text-gray-700 mb-1">专属订阅源 (留空则用Default)</label>
                    <input v-model="authForm.custom_sub_url" type="text" class="w-full px-3 py-2 border rounded focus:outline-none focus:ring-1 focus:ring-indigo-500" placeholder="https://...">
                </div>
                
                <div class="mb-6">
                    <label class="block text-sm font-medium text-gray-700 mb-1">有效时间 (天)</label>
                    <input v-model.number="authForm.days" type="number" class="w-full px-3 py-2 border rounded focus:outline-none focus:ring-1 focus:ring-indigo-500" placeholder="留空或0表示永久有效">
                </div>
                
                <div class="flex justify-end gap-3">
                    <button @click="showModal = false" class="px-4 py-2 text-gray-600 bg-gray-100 rounded hover:bg-gray-200">取消</button>
                    <!-- 吊销按钮 -->
                    <button v-if="currentDevice?.authorized" @click="revokeAuth" class="px-4 py-2 text-white bg-red-600 rounded hover:bg-red-700">吊销授权</button>
                    <button @click="submitAuth" class="px-4 py-2 text-white bg-indigo-600 rounded hover:bg-indigo-700">确认授权</button>
                </div>
            </div>
        </div>
    </div>

    <script>
        const { createApp, ref, onMounted } = Vue;
        
        createApp({
            setup() {
                const isLoggedIn = ref(false);
                const password = ref('');
                const devices = ref([]);
                
                const showModal = ref(false);
                const currentDevice = ref(null);
                const authForm = ref({ display_text: '', days: '', custom_sub_url: '' });

                onMounted(() => {
                    const savedPwd = localStorage.getItem('vpn_admin_pwd');
                    if (savedPwd) {
                        password.value = savedPwd;
                        fetchDevices();
                    }
                });

                const req = async (path, opts = {}) => {
                    const res = await fetch(path, {
                        ...opts,
                        headers: {
                            'Content-Type': 'application/json',
                            'x-admin-password': password.value,
                            ...(opts.headers || {})
                        }
                    });
                    if (res.status === 401) {
                        isLoggedIn.value = false;
                        localStorage.removeItem('vpn_admin_pwd');
                        alert('密码错误或已失效');
                        throw new Error('Unauthorized');
                    }
                    return res.json();
                };

                const login = async () => {
                    try {
                        await fetchDevices();
                        localStorage.setItem('vpn_admin_pwd', password.value);
                    } catch(e) {}
                };

                const logout = () => {
                    localStorage.removeItem('vpn_admin_pwd');
                    isLoggedIn.value = false;
                    password.value = '';
                };

                const fetchDevices = async () => {
                    const data = await req('/api/admin/list');
                    if (data.success) {
                        devices.value = data.data.sort((a, b) => b.last_seen - a.last_seen);
                        isLoggedIn.value = true;
                    }
                };

                const openAuthModal = (device) => {
                    currentDevice.value = device;
                    authForm.value.display_text = device.display_text || 'NarcissusAura VIP';
                    authForm.value.custom_sub_url = device.custom_sub_url || '';
                    
                    if (device.authorized) {
                        if (device.expires_at) {
                            const remainDays = Math.ceil((device.expires_at - Date.now()) / (86400 * 1000));
                            authForm.value.days = remainDays > 0 ? remainDays : 0;
                        } else {
                            authForm.value.days = ''; // 永久有效
                        }
                    } else {
                        authForm.value.days = 30; // 新授权默认建议 30
                    }
                    showModal.value = true;
                };

                const submitAuth = async () => {
                    if (!currentDevice.value) return;
                    await req('/api/admin/update', {
                        method: 'POST',
                        body: JSON.stringify({
                            machine_id: currentDevice.value.machine_id,
                            authorized: true,
                            display_text: authForm.value.display_text,
                            custom_sub_url: authForm.value.custom_sub_url,
                            days: authForm.value.days === '' ? 0 : Number(authForm.value.days)
                        })
                    });
                    showModal.value = false;
                    fetchDevices();
                };

                const revokeAuth = async () => {
                    if (!currentDevice.value) return;
                    await req('/api/admin/update', {
                        method: 'POST',
                        body: JSON.stringify({
                            machine_id: currentDevice.value.machine_id,
                            authorized: false,
                            display_text: '未授权设备',
                            custom_sub_url: '',
                            days: 0
                        })
                    });
                    showModal.value = false;
                    fetchDevices();
                };

                const deleteDevice = async (id) => {
                    if (!confirm('确定删除该请求记录吗？如果该设备再次连接会重新生成待审批记录。')) return;
                    await req('/api/admin/delete', {
                        method: 'POST',
                        body: JSON.stringify({ machine_id: id })
                    });
                    fetchDevices();
                };

                const formatDate = (ts) => {
                    if (!ts) return '-';
                    return new Date(ts).toLocaleString();
                };

                const formatExpires = (ts, authorized) => {
                    if (!authorized) return '-';
                    if (!ts) return '永久有效';
                    const diff = ts - Date.now();
                    if (diff <= 0) return '即将自动销毁';
                    return new Date(ts).toLocaleString();
                };

                const getExpiryClass = (ts) => {
                    if (!ts) return 'text-green-600';
                    const days = (ts - Date.now()) / (86400 * 1000);
                    if (days < 3) return 'text-red-600 font-bold';
                    if (days < 7) return 'text-yellow-600';
                    return 'text-gray-900';
                };

                return {
                    isLoggedIn, password, login, logout,
                    devices, fetchDevices, formatDate, formatExpires, getExpiryClass,
                    showModal, currentDevice, authForm, openAuthModal, submitAuth, revokeAuth, deleteDevice
                };
            }
        }).mount('#app');
    </script>
</body>
</html>
`;

export default {
  async fetch(request, env, ctx) {
    const corsHeaders = {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET,POST,OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type, x-admin-password",
    };

    if (request.method === "OPTIONS") {
      return new Response(null, { headers: corsHeaders });
    }

    const url = new URL(request.url);

    // ==========================================
    // 1. App 客户端鉴权接口
    // ==========================================
    if (url.pathname === "/api/auth" && request.method === "POST") {
      try {
        const body = await request.json();
        const machineId = body.machine_id;
        if (!machineId) {
          return Response.json({ success: false, authorized: false, message: "Missing machine_id" }, { headers: corsHeaders });
        }

        let userData = await env.AUTH_DB.get(machineId, "json");

        if (!userData) {
          userData = { authorized: false, status: "pending", display_text: "", last_seen: Date.now() };
          await env.AUTH_DB.put(machineId, JSON.stringify(userData));
        } else {
          userData.last_seen = Date.now();
          if (!userData.authorized || !userData.expires_at) {
             await env.AUTH_DB.put(machineId, JSON.stringify(userData));
          }
        }

        if (!userData.authorized) {
          return Response.json({
            success: true,
            authorized: false,
            message: "请联系服务商授权",
            display_text: "未授权设备: " + machineId.substring(0, 8),
            machine_id: machineId
          }, { headers: corsHeaders });
        }

        // --- JWT 签名颁发 ---
        const payload = {
          machine_id: machineId,
          authorized: true,
          display_text: userData.display_text || "NarcissusAura VIP",
          custom_sub_url: userData.custom_sub_url || "",
          expires_at: userData.expires_at || null, // CF 中的最终到期时间
          issued_at: Date.now() // 发证时间，App 用它判断 7 天缓存期
        };
        const token = await signJWT(payload);

        return Response.json({
          success: true,
          authorized: true,
          message: "授权成功",
          display_text: payload.display_text,
          token: token
        }, { headers: corsHeaders });

      } catch (e) {
        return Response.json({ success: false, message: e.toString() }, { status: 400, headers: corsHeaders });
      }
    }

    // ==========================================
    // 2. Admin 面板 HTML 页面
    // ==========================================
    if (url.pathname === "/admin" && request.method === "GET") {
      return new Response(ADMIN_HTML, {
        headers: { "Content-Type": "text/html;charset=UTF-8" }
      });
    }

    // ==========================================
    // 3. Admin API (鉴权保护)
    // ==========================================
    if (url.pathname.startsWith("/api/admin/")) {
      const pwd = request.headers.get("x-admin-password");
      if (pwd !== ADMIN_PASSWORD) {
        return Response.json({ success: false, message: "Unauthorized" }, { status: 401, headers: corsHeaders });
      }

      try {
        if (url.pathname === "/api/admin/list") {
          const listInfo = await env.AUTH_DB.list();
          const devices = [];
          for (const key of listInfo.keys) {
            const val = await env.AUTH_DB.get(key.name, "json");
            if (val) {
              devices.push({ machine_id: key.name, ...val });
            }
          }
          return Response.json({ success: true, data: devices }, { headers: corsHeaders });
        }
        
        if (url.pathname === "/api/admin/update" && request.method === "POST") {
          const body = await request.json();
          const { machine_id, authorized, display_text, days } = body;
          
          let putOptions = {};
          let expires_at = null;

          if (days && Number(days) > 0) {
             const seconds = Number(days) * 86400;
             putOptions.expirationTtl = seconds;
             expires_at = Date.now() + (seconds * 1000);
          }

          const newData = { 
            authorized, 
            display_text, 
            status: authorized ? "approved" : "pending", 
            last_seen: Date.now(),
            expires_at
          };
          
          await env.AUTH_DB.put(machine_id, JSON.stringify(newData), putOptions);
          return Response.json({ success: true }, { headers: corsHeaders });
        }

        if (url.pathname === "/api/admin/delete" && request.method === "POST") {
          const body = await request.json();
          await env.AUTH_DB.delete(body.machine_id);
          return Response.json({ success: true }, { headers: corsHeaders });
        }
      } catch (e) {
        return Response.json({ success: false, message: e.toString() }, { status: 500, headers: corsHeaders });
      }
    }

    return new Response("Not Found", { status: 404, headers: corsHeaders });
  }
};
