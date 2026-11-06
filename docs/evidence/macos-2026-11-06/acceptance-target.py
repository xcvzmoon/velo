from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
import time
class Target(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def do_GET(self):
        time.sleep(0.01)
        self.send_response(200)
        self.send_header('Content-Length', '7')
        self.end_headers()
        self.wfile.write(b'payload')
    def log_message(self, *_): pass
class AcceptanceServer(ThreadingHTTPServer):
    request_queue_size = 128

AcceptanceServer(('127.0.0.1', 8080), Target).serve_forever()
