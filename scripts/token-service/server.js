/**
 * Moho IPC Token Service
 * 
 * API:
 *   POST /token/create  - 创建 token
 *   POST /token/verify  - 验证 token
 *   POST /token/revoke  - 撤销 token
 *   GET  /health        - 健康检查
 *   GET  /stats         - 统计信息
 */

const http = require('http');
const crypto = require('crypto');

// 配置
const PORT = process.env.MOHO_TOKEN_PORT || 9527;
const TOKEN_EXPIRE = parseInt(process.env.MOHO_TOKEN_EXPIRE || '3600'); // 默认 1 小时
const TOKEN_LENGTH = 32;

// 内存存储（生产环境可用 Redis）
const tokens = new Map(); // token -> { client_id, user, created_at, expire }

// 允许的 client_id
const ALLOWED_CLIENTS = [
  'moho-mate',
  'moho_ipc_client',
  'openclaw-agent'
];

// 日志
function log(level, message, data = {}) {
  const timestamp = new Date().toISOString();
  console.log(`${timestamp} [${level}] ${message}`, Object.keys(data).length ? data : '');
}

// 生成随机 token
function generateToken(length = TOKEN_LENGTH) {
  return crypto.randomBytes(length).toString('base64url').slice(0, length);
}

// 清理过期 token
function cleanExpiredTokens() {
  const now = Date.now();
  let cleaned = 0;
  
  for (const [token, data] of tokens) {
    if (data.created_at + data.expire * 1000 < now) {
      tokens.delete(token);
      cleaned++;
    }
  }
  
  if (cleaned > 0) {
    log('INFO', `清理过期 token`, { count: cleaned });
  }
}

// 解析 JSON body
function parseBody(req) {
  return new Promise((resolve, reject) => {
    let body = '';
    req.on('data', chunk => body += chunk);
    req.on('end', () => {
      try {
        resolve(body ? JSON.parse(body) : {});
      } catch (e) {
        reject(new Error('Invalid JSON'));
      }
    });
    req.on('error', reject);
  });
}

// 发送 JSON 响应
function sendJson(res, status, data) {
  res.writeHead(status, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(data));
}

// API: 创建 token
async function handleCreateToken(req, res) {
  try {
    const body = await parseBody(req);
    const { client_id, user } = body;
    
    // 验证 client_id
    if (!client_id) {
      return sendJson(res, 400, { error: 'missing client_id' });
    }
    
    if (!ALLOWED_CLIENTS.includes(client_id)) {
      log('WARN', `拒绝未授权的 client_id`, { client_id });
      return sendJson(res, 403, { error: 'unauthorized client_id' });
    }
    
    // 生成 token
    const token = generateToken();
    const expire = body.expire || TOKEN_EXPIRE;
    
    // 存储
    tokens.set(token, {
      client_id,
      user: user || 'unknown',
      created_at: Date.now(),
      expire
    });
    
    log('INFO', `Token 已创建`, { client_id, user, expire });
    
    sendJson(res, 200, {
      token,
      expire,
      created_at: new Date().toISOString()
    });
    
  } catch (e) {
    log('ERROR', `创建 token 失败`, { error: e.message });
    sendJson(res, 500, { error: e.message });
  }
}

// API: 验证 token
async function handleVerifyToken(req, res) {
  try {
    const body = await parseBody(req);
    const { token } = body;
    
    if (!token) {
      return sendJson(res, 400, { error: 'missing token' });
    }
    
    const data = tokens.get(token);
    
    if (!data) {
      log('WARN', `Token 不存在`, { token: token.slice(0, 8) + '...' });
      return sendJson(res, 200, { valid: false, reason: 'not_found' });
    }
    
    // 检查过期
    const now = Date.now();
    if (data.created_at + data.expire * 1000 < now) {
      tokens.delete(token);
      log('WARN', `Token 已过期`, { token: token.slice(0, 8) + '...' });
      return sendJson(res, 200, { valid: false, reason: 'expired' });
    }
    
    log('INFO', `Token 验证成功`, { client_id: data.client_id });
    
    sendJson(res, 200, {
      valid: true,
      client_id: data.client_id,
      user: data.user,
      remaining_seconds: Math.floor((data.created_at + data.expire * 1000 - now) / 1000)
    });
    
  } catch (e) {
    log('ERROR', `验证 token 失败`, { error: e.message });
    sendJson(res, 500, { error: e.message });
  }
}

// API: 撤销 token
async function handleRevokeToken(req, res) {
  try {
    const body = await parseBody(req);
    const { token } = body;
    
    if (!token) {
      return sendJson(res, 400, { error: 'missing token' });
    }
    
    const deleted = tokens.delete(token);
    
    log('INFO', `Token 撤销`, { token: token.slice(0, 8) + '...', deleted });
    
    sendJson(res, 200, { success: deleted });
    
  } catch (e) {
    log('ERROR', `撤销 token 失败`, { error: e.message });
    sendJson(res, 500, { error: e.message });
  }
}

// API: 健康检查
function handleHealth(req, res) {
  sendJson(res, 200, {
    status: 'ok',
    uptime: process.uptime(),
    tokens_active: tokens.size
  });
}

// API: 统计信息
function handleStats(req, res) {
  const stats = {
    total_tokens: tokens.size,
    by_client: {},
    by_user: {}
  };
  
  for (const [_, data] of tokens) {
    stats.by_client[data.client_id] = (stats.by_client[data.client_id] || 0) + 1;
    stats.by_user[data.user] = (stats.by_user[data.user] || 0) + 1;
  }
  
  sendJson(res, 200, stats);
}

// 路由
async function handleRequest(req, res) {
  const { method, url } = req;
  const path = url.split('?')[0];
  
  // CORS headers（仅允许本地访问）
  res.setHeader('Access-Control-Allow-Origin', 'http://127.0.0.1:*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
  
  // OPTIONS 预检
  if (method === 'OPTIONS') {
    return sendJson(res, 200, {});
  }
  
  // 路由分发
  if (method === 'POST' && path === '/token/create') {
    return handleCreateToken(req, res);
  }
  
  if (method === 'POST' && path === '/token/verify') {
    return handleVerifyToken(req, res);
  }
  
  if (method === 'POST' && path === '/token/revoke') {
    return handleRevokeToken(req, res);
  }
  
  if (method === 'GET' && path === '/health') {
    return handleHealth(req, res);
  }
  
  if (method === 'GET' && path === '/stats') {
    return handleStats(req, res);
  }
  
  // 404
  sendJson(res, 404, { error: 'not found' });
}

// 启动服务
const server = http.createServer(handleRequest);

// 定期清理过期 token
setInterval(cleanExpiredTokens, 60000); // 每分钟清理一次

server.listen(PORT, '127.0.0.1', () => {
  log('INFO', `Token 服务启动`, { port: PORT });
  console.log(`
╔═══════════════════════════════════════════════════════════╗
║              Moho IPC Token Service                       ║
╠═══════════════════════════════════════════════════════════╣
║  端口: ${PORT}                                              ║
║  Token 有效期: ${TOKEN_EXPIRE} 秒                                ║
╠═══════════════════════════════════════════════════════════╣
║  API:                                                      ║
║    POST /token/create  - 创建 token                        ║
║    POST /token/verify  - 验证 token                        ║
║    POST /token/revoke  - 撤销 token                        ║
║    GET  /health        - 健康检查                          ║
║    GET  /stats         - 统计信息                          ║
╠═══════════════════════════════════════════════════════════╣
║  示例:                                                     ║
║    curl -X POST http://127.0.0.1:${PORT}/token/create \\    ║
║      -H "Content-Type: application/json" \\                ║
║      -d '{"client_id":"moho-mate","user":"def"}'           ║
╚═══════════════════════════════════════════════════════════╝
`);
});

// 优雅退出
process.on('SIGTERM', () => {
  log('INFO', '收到 SIGTERM，正在关闭...');
  server.close(() => {
    log('INFO', '服务已关闭');
    process.exit(0);
  });
});

process.on('SIGINT', () => {
  log('INFO', '收到 SIGINT，正在关闭...');
  server.close(() => {
    log('INFO', '服务已关闭');
    process.exit(0);
  });
});
