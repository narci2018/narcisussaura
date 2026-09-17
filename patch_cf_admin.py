import re

with open('server/src/index.js', 'r', encoding='utf-8') as f:
    content = f.read()

# Fix /api/admin/update to capture and save custom_sub_url
admin_update_patch = r'''        if (url.pathname === "/api/admin/update" && request.method === "POST") {
          const body = await request.json();
          const { machine_id, authorized, display_text, days, custom_sub_url } = body;
          
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
            custom_sub_url,
            status: authorized ? "approved" : "pending", 
            last_seen: Date.now(),
            expires_at
          };'''
          
content = re.sub(
    r'''        if \(url\.pathname === "/api/admin/update" && request\.method === "POST"\) \{\n          const body = await request\.json\(\);\n          const \{ machine_id, authorized, display_text, days \} = body;\n          \n          let putOptions = \{\};\n          let expires_at = null;\n\n          if \(days && Number\(days\) > 0\) \{\n             const seconds = Number\(days\) \* 86400;\n             putOptions\.expirationTtl = seconds;\n             expires_at = Date\.now\(\) \+ \(seconds \* 1000\);\n          \}\n\n          const newData = \{ \n            authorized, \n            display_text, \n            status: authorized \? "approved" : "pending", \n            last_seen: Date\.now\(\),\n            expires_at\n          \};''',
    admin_update_patch,
    content
)

with open('server/src/index.js', 'w', encoding='utf-8') as f:
    f.write(content)
