#!/usr/bin/env python3
"""
用法:
    python server.py
然后浏览器打开 http://localhost:8080
"""
from http.server import HTTPServer, BaseHTTPRequestHandler
import urllib.request, urllib.parse, os

PORT = 8080
BASE_DIR = os.path.dirname(os.path.abspath(__file__))

def make_headers(target_url):
    parsed  = urllib.parse.urlparse(target_url)
    referer = f"{parsed.scheme}://{parsed.netloc}/"
    return {
        "User-Agent":      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        "Referer":         referer,
        "Accept":          "text/html,application/json,*/*",
        "Accept-Language": "zh-CN,zh;q=0.9",
    }

class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print("[%s] %s" % (self.address_string(), fmt % args))

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        qs     = urllib.parse.parse_qs(parsed.query)

        static = {
            "/":                "nnyy_player.html",
            "/nnyy_player.html":"nnyy_player.html",
            "/history":         "history.html",
            "/history.html":    "history.html",
        }
        if parsed.path in static:
            path = os.path.join(BASE_DIR, static[parsed.path])
            try:
                with open(path, "rb") as f:
                    body = f.read()
                self._respond(200, "text/html; charset=utf-8", body)
            except FileNotFoundError:
                self.send_error(404, f"{static[parsed.path]} not found")
            return

        if parsed.path == "/proxy":
            target = qs.get("url", [""])[0]
            if not target:
                self.send_error(400, "Missing url param"); return
            try:
                req = urllib.request.Request(target, headers=make_headers(target))
                with urllib.request.urlopen(req, timeout=20) as r:
                    body = r.read()
                    ct   = r.headers.get("Content-Type", "application/octet-stream")
                self._respond(200, ct, body)
            except (ConnectionAbortedError, BrokenPipeError):
                pass  # 客户端主动关闭连接，忽略
            except urllib.error.HTTPError as e:
                try: self.send_error(e.code, "Upstream HTTP error")
                except (ConnectionAbortedError, BrokenPipeError): pass
            except Exception as e:
                try: self.send_error(502, "Bad Gateway")
                except (ConnectionAbortedError, BrokenPipeError): pass
            return

        self.send_error(404)

    def _respond(self, code, ct, body):
        try:
            self.send_response(code)
            self.send_header("Content-Type",   ct)
            self.send_header("Content-Length", len(body))
            self.send_header("Access-Control-Allow-Origin", "*")
            self.end_headers()
            self.wfile.write(body)
        except (ConnectionAbortedError, BrokenPipeError):
            pass

if __name__ == "__main__":
    srv = HTTPServer(("127.0.0.1", PORT), Handler)
    print(f"✓ 代理服务器已启动")
    print(f"  浏览器打开 → http://localhost:{PORT}")
    print(f"  Ctrl+C 停止\n")
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        print("\n已停止")
