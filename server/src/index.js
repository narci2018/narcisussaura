export default {
  async fetch(request, env, ctx) {
    const corsHeaders = {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET,POST,OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type",
    };

    if (request.method === "OPTIONS") {
      return new Response(null, { headers: corsHeaders });
    }

    const url = new URL(request.url);

    // Provide a simple healthcheck root endpoint
    if (url.pathname === "/") {
      return Response.json({ status: "ok", service: "vpn-auth-server" }, { headers: corsHeaders });
    }

    if (url.pathname === "/api/auth" && request.method === "POST") {
      try {
        const body = await request.json();
        const machineId = body.machine_id;

        if (!machineId) {
          return Response.json({ 
            success: false, 
            authorized: false, 
            message: "请联系服务商授权", 
            display_text: "获取机器码失败" 
          }, { headers: corsHeaders });
        }

        // Search KV for authorization
        const userData = await env.AUTH_DB.get(machineId, "json");

        if (!userData || !userData.authorized) {
          return Response.json({
            success: true,
            authorized: false,
            message: "请联系服务商授权",
            display_text: "未授权设备: " + machineId.substring(0, 8),
            machine_id: machineId
          }, { headers: corsHeaders });
        }

        return Response.json({
          success: true,
          authorized: true,
          message: "授权成功",
          display_text: userData.display_text || "NarcissusAura VIP"
        }, { headers: corsHeaders });

      } catch (e) {
        return Response.json({ 
          success: false, 
          authorized: false,
          message: "服务器错误: " + e.toString() 
        }, { status: 400, headers: corsHeaders });
      }
    }

    return new Response("Not Found", { status: 404, headers: corsHeaders });
  }
};
