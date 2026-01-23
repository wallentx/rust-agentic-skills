# MCP Fix Summary

## Why it failed
- Gemini CLI sends line-delimited JSON-RPC over stdio during MCP discovery, not framed `Content-Length` messages.
- The server initially expected only MCP framing, so it waited for headers and never replied to Gemini's line-delimited initialize request.
- JSON-RPC responses also included `"error": null`, which is not valid per JSON-RPC 2.0; strict clients can reject such responses.

## What changed
- Added dual-mode transport: detect line-delimited JSON requests and respond with newline JSON, while still supporting framed MCP messages.
- Implemented MCP framing support (Content-Length parsing + framed responses) for compliant MCP clients.
- Made JSON-RPC responses strict by omitting `result`/`error` when absent.
- Added MCP discovery handlers (`resources/list`, `resources/read`, `logging/setLevel`) and echoed the client's protocol version in `initialize`.

## Result
- Gemini CLI now completes MCP discovery and no longer hangs at initialization.
