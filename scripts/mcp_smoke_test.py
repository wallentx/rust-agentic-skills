#!/usr/bin/env python3
import json
import subprocess
import sys
import threading
import time

def send_frame(proc, payload):
    data = payload.encode("utf-8")
    header = f"Content-Length: {len(data)}\r\n\r\n".encode("utf-8")
    proc.stdin.write(header + data)
    proc.stdin.flush()

def read_frame(proc, timeout_s=5.0):
    headers = b""
    deadline = time.time() + timeout_s
    while b"\r\n\r\n" not in headers:
        if time.time() > deadline:
            return None
        chunk = proc.stdout.read(1)
        if not chunk:
            return None
        headers += chunk
    header_text, rest = headers.split(b"\r\n\r\n", 1)
    content_length = None
    for line in header_text.decode("utf-8").split("\r\n"):
        if line.lower().startswith("content-length:"):
            content_length = int(line.split(":", 1)[1].strip())
            break
    if content_length is None:
        return None
    while len(rest) < content_length:
        if time.time() > deadline:
            return None
        chunk = proc.stdout.read(content_length - len(rest))
        if not chunk:
            return None
        rest += chunk
    return rest[:content_length].decode("utf-8")

def main():
    bin_path = sys.argv[1] if len(sys.argv) > 1 else "./target/release/rust-agentic-skills"
    proc = subprocess.Popen(
        [bin_path],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    def drain_stderr():
        for line in iter(proc.stderr.readline, b""):
            sys.stderr.write(line.decode("utf-8"))
            sys.stderr.flush()

    t = threading.Thread(target=drain_stderr, daemon=True)
    t.start()

    requests = [
        {"jsonrpc": "2.0", "method": "initialize", "params": {}, "id": 1},
        {"jsonrpc": "2.0", "method": "notifications/initialized"},
        {"jsonrpc": "2.0", "method": "tools/list", "id": 2},
        {"jsonrpc": "2.0", "method": "prompts/list", "id": 3},
        {"jsonrpc": "2.0", "method": "resources/list", "id": 4},
        {"jsonrpc": "2.0", "method": "ping", "id": 5},
    ]

    for req in requests:
        send_frame(proc, json.dumps(req))
        if "id" in req:
            resp = read_frame(proc, timeout_s=5.0)
            print(resp)

    proc.stdin.close()
    proc.terminate()

if __name__ == "__main__":
    main()
