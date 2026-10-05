#!/usr/bin/env python3
"""Remote goose ACP client over WebSocket.

Talks to `goose serve`. The secret is never stored in this file. Pass it with
--secret, or export GOOSE_SERVER__SECRET_KEY (the same variable `goose serve`
reads). This client sends it as the X-Secret-Key header. A ?token= query
parameter is accepted and then removed, so the secret is not left on the URL
that libraries print when a connection fails.

    pip install websockets
    set GOOSE_SERVER__SECRET_KEY=your-secret
    python scripts/acp_remote_client.py --host 127.0.0.1 "your prompt"

    python scripts/acp_remote_client.py --url ws://127.0.0.1:3284/acp --cwd C:\\work --timeout 900 -
"""

import argparse
import asyncio
import base64
import hashlib
import json
import os
import sys
from pathlib import Path
from urllib.parse import parse_qsl, urlencode, urlsplit, urlunsplit

import websockets

DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 3284
DEFAULT_TIMEOUT_SECONDS = 900
SECRET_ENV = "GOOSE_SERVER__SECRET_KEY"
MAX_MESSAGE_BYTES = 64 * 1024 * 1024
MIME_EXTENSIONS = {
    "image/jpeg": ".jpg",
    "image/jpg": ".jpg",
    "image/png": ".png",
    "image/gif": ".gif",
    "image/webp": ".webp",
}


def text_of(content):
    if isinstance(content, dict):
        if isinstance(content.get("text"), str):
            return content["text"]
        nested = content.get("content")
        if nested is not None and nested is not content:
            return text_of(nested)
        return ""
    if isinstance(content, list):
        return "".join(text_of(part) for part in content)
    if isinstance(content, str):
        return content
    return ""


def nonempty(value):
    if value is None:
        return None
    value = value.strip()
    return value or None


def redact(text, secret):
    if secret and secret in text:
        return text.replace(secret, "REDACTED")
    return text


def image_payloads(node):
    found = []

    def walk(value):
        if isinstance(value, dict):
            mime = value.get("mimeType") or value.get("mime_type") or ""
            if isinstance(mime, str) and mime.lower().startswith("image/"):
                payload = value.get("data")
                if not isinstance(payload, str):
                    payload = value.get("blob")
                if isinstance(payload, str) and payload.strip():
                    found.append((mime.lower().split(";", 1)[0].strip(), payload))
            for child in value.values():
                walk(child)
        elif isinstance(value, list):
            for child in value:
                walk(child)

    walk(node)
    return found


def save_image(directory, mime, payload, seen):
    try:
        raw = base64.b64decode(payload, validate=False)
    except Exception:
        return None
    if not raw:
        return None
    digest = hashlib.sha256(raw).hexdigest()
    if digest in seen:
        return None
    seen.add(digest)
    directory.mkdir(parents=True, exist_ok=True)
    extension = MIME_EXTENSIONS.get(mime, ".img")
    path = directory / f"{len(seen):03d}-{digest[:12]}{extension}"
    path.write_bytes(raw)
    return path, len(raw), mime, digest


def build_url(args):
    if args.url:
        return args.url
    scheme = "wss" if args.tls else "ws"
    return f"{scheme}://{args.host}:{args.port}/acp"


def split_token(url):
    parts = urlsplit(url)
    kept = []
    token = None
    for key, value in parse_qsl(parts.query, keep_blank_values=True):
        if key == "token" and token is None:
            token = nonempty(value)
            continue
        kept.append((key, value))
    path = parts.path or "/acp"
    clean = urlunsplit((parts.scheme, parts.netloc, path, urlencode(kept), ""))
    return clean, token


def resolve_secret(explicit, url_token):
    return nonempty(explicit) or nonempty(os.environ.get(SECRET_ENV)) or url_token


def resolve_prompt(args):
    sources = [args.prompt is not None, args.prompt_file is not None]
    if sum(sources) > 1:
        raise ValueError("pass the prompt as an argument or with --prompt-file, not both")
    if args.prompt_file:
        if args.prompt_file == "-":
            return sys.stdin.read()
        with open(args.prompt_file, encoding="utf-8") as handle:
            return handle.read()
    if args.prompt == "-":
        return sys.stdin.read()
    if args.prompt is not None:
        return args.prompt
    if not sys.stdin.isatty():
        return sys.stdin.read()
    raise ValueError("prompt is required; pass it as an argument, --prompt-file, or stdin")


def choose_option(options, mode):
    if mode == "deny" or not options:
        return None
    preferred = (
        ("allow_always", "allow_once")
        if mode == "allow-always"
        else ("allow_once", "allow_always")
    )
    for kind in preferred:
        chosen = next((opt for opt in options if opt.get("kind") == kind), None)
        if chosen:
            return chosen
    return options[0]


def cwd_for(sessions, session_id):
    if session_id:
        match = next((session for session in sessions if session.get("sessionId") == session_id), None)
        if match and match.get("cwd"):
            return match["cwd"]
    return next((session.get("cwd") for session in sessions if session.get("cwd")), None)


def parse_args(argv):
    parser = argparse.ArgumentParser(
        description="Send one prompt to a remote goose ACP server over WebSocket.",
    )
    parser.add_argument(
        "prompt",
        nargs="?",
        help="Prompt text. Use - to read stdin. Also read from stdin when it is not a terminal.",
    )
    parser.add_argument(
        "--url",
        help="WebSocket URL, for example ws://127.0.0.1:3284/acp. A ?token= value is used as the secret and then removed.",
    )
    parser.add_argument("--host", default=DEFAULT_HOST, help=f"Host used when --url is omitted (default: {DEFAULT_HOST})")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT, help=f"Port used when --url is omitted (default: {DEFAULT_PORT})")
    parser.add_argument("--tls", action="store_true", help="Use wss:// when --url is omitted")
    parser.add_argument(
        "--secret",
        help=f"Server secret. Defaults to ${SECRET_ENV}. Sent as the X-Secret-Key header.",
    )
    parser.add_argument("--cwd", help="Working directory for session/new or session/load")
    parser.add_argument("--session", help="Load this session instead of creating a new one")
    parser.add_argument(
        "--timeout",
        type=float,
        default=DEFAULT_TIMEOUT_SECONDS,
        help=f"Seconds to wait for session/prompt (default: {DEFAULT_TIMEOUT_SECONDS})",
    )
    parser.add_argument(
        "--permission",
        choices=("allow-always", "allow-once", "deny"),
        default="allow-always",
        help="How to answer session/request_permission (default: allow-always)",
    )
    parser.add_argument("--prompt-file", help="Read the prompt from this file. Use - for stdin.")
    parser.add_argument(
        "--image-dir",
        help="Write image bytes received over ACP into this directory.",
    )
    return parser.parse_args(argv)


async def main(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        prompt = resolve_prompt(args).strip()
    except ValueError as exc:
        print(exc, file=sys.stderr)
        return 2
    except OSError as exc:
        print(f"failed to read prompt: {exc}", file=sys.stderr)
        return 2
    if not prompt:
        print("prompt is empty", file=sys.stderr)
        return 2

    url, url_token = split_token(build_url(args))
    secret = resolve_secret(args.secret, url_token)
    headers = {"X-Secret-Key": secret} if secret else None

    assistant = []
    tools = []
    saved_images = []
    seen_images = set()
    pending = {}
    next_id = 1
    image_dir = Path(args.image_dir) if args.image_dir else None

    def keep_images(node):
        for mime, payload in image_payloads(node):
            if image_dir is None:
                print(f"\n[image] {mime} received; pass --image-dir to save it", flush=True)
                continue
            saved = save_image(image_dir, mime, payload, seen_images)
            if saved is None:
                continue
            path, size, saved_mime, digest = saved
            saved_images.append((path, size, saved_mime, digest))
            print(f"\n[image] {path} {size} bytes {saved_mime}", flush=True)

    print(f"connecting {url}", flush=True)
    connection = websockets.connect(
        url,
        additional_headers=headers,
        open_timeout=10,
        max_size=MAX_MESSAGE_BYTES,
        ping_interval=20,
        ping_timeout=60,
    )
    try:
        ws = await connection.__aenter__()
    except websockets.exceptions.InvalidStatus as exc:
        if exc.response.status_code == 401:
            print(
                f"authentication failed (401). Pass --secret or set {SECRET_ENV}.",
                file=sys.stderr,
            )
        else:
            print(f"connection failed: {redact(str(exc), secret)}", file=sys.stderr)
        return 1
    except (OSError, websockets.exceptions.InvalidHandshake, websockets.exceptions.InvalidURI) as exc:
        print(f"connection failed: {redact(str(exc), secret)}", file=sys.stderr)
        return 1

    try:
        async def send_result(msg_id, result):
            await ws.send(json.dumps({"jsonrpc": "2.0", "id": msg_id, "result": result}))

        async def send_error(msg_id, message):
            await ws.send(
                json.dumps(
                    {
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "error": {"code": -32601, "message": message},
                    }
                )
            )

        async def handle_request(msg):
            method = msg.get("method")
            params = msg.get("params") or {}
            print(f"\n[client-request] {method}", flush=True)
            if method != "session/request_permission":
                await send_error(msg["id"], f"client does not implement {method}")
                return

            options = params.get("options") or []
            chosen = choose_option(options, args.permission)
            tool = (params.get("toolCall") or {}).get("title") or (
                params.get("toolCall") or {}
            ).get("toolCallId")
            if chosen:
                print(f"[permission] allow {tool} via {chosen.get('kind')}", flush=True)
                await send_result(
                    msg["id"],
                    {"outcome": {"outcome": "selected", "optionId": chosen["optionId"]}},
                )
            else:
                print(f"[permission] deny {tool}", flush=True)
                await send_result(msg["id"], {"outcome": {"outcome": "cancelled"}})

        def handle_note(msg):
            update = (msg.get("params") or {}).get("update") or {}
            kind = update.get("sessionUpdate") or msg.get("method")
            if kind == "agent_message_chunk":
                chunk = text_of(update.get("content"))
                if chunk:
                    assistant.append(chunk)
                    print(chunk, end="", flush=True)
            elif kind == "tool_call":
                title = update.get("title") or update.get("kind") or update.get("toolCallId")
                tools.append(str(title))
                print(f"\n[tool] {title}", flush=True)
                raw_input = update.get("rawInput")
                if raw_input is not None:
                    preview = json.dumps(raw_input, ensure_ascii=False)
                    if len(preview) > 400:
                        preview = preview[:400] + "…"
                    print(f"[tool args] {preview}", flush=True)
            elif kind == "tool_call_update" and update.get("status") in (
                "completed",
                "failed",
                "cancelled",
            ):
                print(
                    f"\n[tool {update.get('status')}] {update.get('title') or update.get('toolCallId')}",
                    flush=True,
                )
                excerpt = text_of(update.get("content")).strip().replace("\n", " ")
                if excerpt:
                    print(f"[tool text] {excerpt[:300]}", flush=True)
            elif kind not in (
                "user_message_chunk",
                "agent_thought_chunk",
                "usage_update",
                "session_info_update",
                "available_commands_update",
            ):
                print(f"\n[note] {kind}", flush=True)
            keep_images(update)

        async def reader():
            async for raw in ws:
                msg = json.loads(raw)
                if "method" in msg and "id" in msg:
                    await handle_request(msg)
                    continue
                if "id" in msg and msg["id"] in pending:
                    fut = pending.pop(msg["id"])
                    if not fut.done():
                        fut.set_result(msg)
                    continue
                handle_note(msg)

        reader_task = asyncio.create_task(reader())

        async def rpc(method, params, timeout):
            nonlocal next_id
            req_id = next_id
            next_id += 1
            fut = asyncio.get_running_loop().create_future()
            pending[req_id] = fut
            await ws.send(
                json.dumps(
                    {"jsonrpc": "2.0", "id": req_id, "method": method, "params": params}
                )
            )
            try:
                return await asyncio.wait_for(fut, timeout)
            except TimeoutError:
                pending.pop(req_id, None)
                raise

        try:
            init = await rpc(
                "initialize",
                {
                    "protocolVersion": 1,
                    "clientCapabilities": {"loadSession": True},
                    "clientInfo": {"name": "acp-remote-client", "version": "1.0.0"},
                },
                20,
            )
            if "error" in init:
                print("initialize failed", json.dumps(init["error"], ensure_ascii=False), file=sys.stderr)
                return 1
            info = init["result"]["agentInfo"]
            print(f"agent {info['name']} {info['version']}", flush=True)

            listed = await rpc("session/list", {}, 20)
            if "error" in listed:
                print("session/list failed", json.dumps(listed["error"], ensure_ascii=False), file=sys.stderr)
                return 1
            sessions = listed["result"].get("sessions") or []
            cwd = args.cwd or cwd_for(sessions, args.session)
            if not cwd:
                print("no cwd available; pass --cwd", file=sys.stderr)
                return 1
            print(f"using cwd {cwd}", flush=True)

            if args.session:
                opened = await rpc(
                    "session/load",
                    {"sessionId": args.session, "cwd": cwd, "mcpServers": []},
                    180,
                )
                if "error" in opened:
                    print("session/load failed", json.dumps(opened["error"], ensure_ascii=False), file=sys.stderr)
                    return 1
                session_id = args.session
                print(f"loaded session {session_id}", flush=True)
                await ws.send(
                    json.dumps(
                        {
                            "jsonrpc": "2.0",
                            "method": "session/cancel",
                            "params": {"sessionId": session_id},
                        }
                    )
                )
                print("cancelled any in-flight prompt", flush=True)
                await asyncio.sleep(1)
            else:
                opened = await rpc("session/new", {"cwd": cwd, "mcpServers": []}, 60)
                if "error" in opened:
                    print("session/new failed", json.dumps(opened["error"], ensure_ascii=False), file=sys.stderr)
                    return 1
                session_id = opened["result"]["sessionId"]
                print(f"session {session_id}", flush=True)

            print("--- prompt ---", flush=True)
            print(prompt, flush=True)

            try:
                result = await rpc(
                    "session/prompt",
                    {
                        "sessionId": session_id,
                        "prompt": [{"type": "text", "text": prompt}],
                    },
                    args.timeout,
                )
            except TimeoutError:
                print(
                    f"\nprompt timed out after {args.timeout:g}s; pass a larger --timeout",
                    file=sys.stderr,
                )
                return 1
            print("\n--- done ---", flush=True)
            if "error" in result:
                print("prompt error", json.dumps(result["error"], ensure_ascii=False)[:2000], file=sys.stderr)
                return 1
            print("stopReason", result["result"].get("stopReason"))
            print("tools", len(tools))
            print("assistant_chars", sum(len(chunk) for chunk in assistant))
            print("images", len(saved_images))
            for path, size, saved_mime, _digest in saved_images:
                print(f"saved {path} {size} {saved_mime}")
            return 0
        finally:
            reader_task.cancel()
    finally:
        await connection.__aexit__(None, None, None)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")
    try:
        raise SystemExit(asyncio.run(main()))
    except KeyboardInterrupt:
        raise SystemExit(130)
