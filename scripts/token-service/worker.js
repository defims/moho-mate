/**
 * Moho IPC Token Service - Cloudflare Worker 版本
 * 
 * 部署步骤:
 *   1. npm install -g wrangler
 *   2. wrangler login
 *   3. wrangler deploy
 * 
 * API:
 *   POST /token/create  - 创建 token
 *   POST /token/verify  - 验证 token
 *   POST /token/revoke  - 撤销 token
 *   GET  /health        - 健康检查
 */

export default {
  // 配置
  config: {
    TOKEN_EXPIRE: 3600,  // 1 小时
    TOKEN_LENGTH: 32,
    ALLOWED_CLIENTS: [
      'moho-mate',
      'moho_ipc_client',
      'openclaw-agent'
    ]
  },

  // KV 存储（需要在 wrangler.toml 中配置）
  // [[kv_namespaces]]
  // binding = "TOKENS"

  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    const { method } = request;
    const path = url.pathname;

    // CORS
    if (method === 'OPTIONS') {
      return this.json(200, {});
    }

    // 路由
    if (method === 'POST' && path === '/token/create') {
      return this.handleCreateToken(request, env);
    }
    
    if (method === 'POST' && path === '/token/verify') {
      return this.handleVerifyToken(request, env);
    }
    
    if (method === 'POST' && path === '/token/revoke') {
      return this.handleRevokeToken(request, env);
    }
    
    if (method === 'GET' && path === '/health') {
      return this.handleHealth(request, env);
    }

    return this.json(404, { error: 'not found' });
  },

  // 生成随机 token
  generateToken(length) {
    const bytes = new Uint8Array(length);
    crypto.getRandomValues(bytes);
    return Array.from(bytes)
      .map(b => b.toString(16).padStart(2, '0'))
      .join('')
      .slice(0, length);
  },

  // JSON 响应
  json(status, data) {
    return new Response(JSON.stringify(data), {
      status,
      headers: {
        'Content-Type': 'application/json',
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
        'Access-Control-Allow-Headers': 'Content-Type'
      }
    });
  },

  // 创建 token
  async handleCreateToken(request, env) {
    try {
      const body = await request.json();
      const { client_id, user, expire } = body;

      // 验证 client_id
      if (!client_id) {
        return this.json(400, { error: 'missing client_id' });
      }

      if (!this.config.ALLOWED_CLIENTS.includes(client_id)) {
        console.warn(`拒绝未授权的 client_id: ${client_id}`);
        return this.json(403, { error: 'unauthorized client_id' });
      }

      // 生成 token
      const token = this.generateToken(this.config.TOKEN_LENGTH);
      const tokenExpire = expire || this.config.TOKEN_EXPIRE;
      const createdAt = Date.now();

      // 存储（使用 KV）
      if (env.TOKENS) {
        await env.TOKENS.put(token, JSON.stringify({
          client_id,
          user: user || 'unknown',
          created_at: createdAt,
          expire: tokenExpire
        }), {
          expirationTtl: tokenExpire // KV 自动过期
        });
      } else {
        // 无 KV 时使用内存（仅用于开发测试）
        console.warn('KV 未配置，使用内存存储');
      }

      console.log(`Token 已创建: ${client_id} ${user}`);

      return this.json(200, {
        token,
        expire: tokenExpire,
        created_at: new Date(createdAt).toISOString()
      });

    } catch (e) {
      console.error(`创建 token 失败: ${e.message}`);
      return this.json(500, { error: e.message });
    }
  },

  // 验证 token
  async handleVerifyToken(request, env) {
    try {
      const body = await request.json();
      const { token } = body;

      if (!token) {
        return this.json(400, { error: 'missing token' });
      }

      // 从 KV 读取
      if (!env.TOKENS) {
        return this.json(200, { valid: false, reason: 'storage_unavailable' });
      }

      const data = await env.TOKENS.get(token);

      if (!data) {
        return this.json(200, { valid: false, reason: 'not_found' });
      }

      const parsed = JSON.parse(data);

      // 检查过期（KV 已自动处理，这里做双重检查）
      const now = Date.now();
      if (parsed.created_at + parsed.expire * 1000 < now) {
        await env.TOKENS.delete(token);
        return this.json(200, { valid: false, reason: 'expired' });
      }

      return this.json(200, {
        valid: true,
        client_id: parsed.client_id,
        user: parsed.user,
        remaining_seconds: Math.floor((parsed.created_at + parsed.expire * 1000 - now) / 1000)
      });

    } catch (e) {
      console.error(`验证 token 失败: ${e.message}`);
      return this.json(500, { error: e.message });
    }
  },

  // 撤销 token
  async handleRevokeToken(request, env) {
    try {
      const body = await request.json();
      const { token } = body;

      if (!token) {
        return this.json(400, { error: 'missing token' });
      }

      if (!env.TOKENS) {
        return this.json(200, { success: false, reason: 'storage_unavailable' });
      }

      await env.TOKENS.delete(token);

      return this.json(200, { success: true });

    } catch (e) {
      console.error(`撤销 token 失败: ${e.message}`);
      return this.json(500, { error: e.message });
    }
  },

  // 健康检查
  async handleHealth(request, env) {
    return this.json(200, {
      status: 'ok',
      storage: env.TOKENS ? 'kv' : 'none'
    });
  }
};
