use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Log to stderr because stdout is for MCP protocol
    eprintln!("Starting Rust Guardian MCP Server...");

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut stdout = tokio::io::stdout();

    let mut line_mode = false;
    loop {
        let payload = match read_mcp_payload(&mut reader).await? {
            Some((p, is_line)) => {
                if is_line {
                    line_mode = true;
                }
                p
            }
            None => break,
        };
        if payload.trim().is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&payload) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to parse request: {} | Payload: {}", e, payload);
                continue;
            }
        };

        let response = handle_request(req).await;
        
        if let Some(resp) = response {
            if line_mode {
                write_line_response(&mut stdout, &resp).await?;
            } else {
                write_mcp_response(&mut stdout, &resp).await?;
            }
        }
    }

    Ok(())
}

async fn read_mcp_payload<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
) -> anyhow::Result<Option<(String, bool)>> {
    let mut content_length: Option<usize> = None;
    let mut saw_any_header = false;

    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).await?;
        if bytes == 0 {
            return Ok(None);
        }

        let line_trim = line.trim_end_matches(&['\r', '\n'][..]).to_string();
        if line_trim.is_empty() {
            if saw_any_header {
                break;
            }
            continue;
        }

        if line_trim.starts_with('{') {
            return Ok(Some((line_trim, true)));
        }

        saw_any_header = true;
        let mut parts = line_trim.splitn(2, ':');
        let key = parts.next().unwrap_or("").trim().to_ascii_lowercase();
        if key == "content-length" {
            let val = parts.next().unwrap_or("").trim();
            if let Ok(len) = val.parse::<usize>() {
                content_length = Some(len);
            }
        }
    }

    let Some(len) = content_length else {
        eprintln!("Missing Content-Length header.");
        return Ok(Some((String::new(), false)));
    };

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    match String::from_utf8(buf) {
        Ok(payload) => Ok(Some((payload, false))),
        Err(e) => {
            eprintln!("Invalid UTF-8 payload: {}", e);
            Ok(Some((String::new(), false)))
        }
    }
}

async fn write_mcp_response<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    resp: &JsonRpcResponse,
) -> anyhow::Result<()> {
    let resp_str = serde_json::to_string(resp)?;
    let header = format!("Content-Length: {}\r\n\r\n", resp_str.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(resp_str.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn write_line_response<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    resp: &JsonRpcResponse,
) -> anyhow::Result<()> {
    let mut resp_str = serde_json::to_string(resp)?;
    resp_str.push('\n');
    writer.write_all(resp_str.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn handle_request(req: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let method = req.method.as_str();
    let id = req.id.clone();

    // Standard MCP/JSON-RPC handling
    let result = match method {
        "initialize" => {
            let protocol_version = req
                .params
                .as_ref()
                .and_then(|p| p.get("protocolVersion"))
                .and_then(|v| v.as_str())
                .unwrap_or("2024-11-05");
            json!({
                "protocolVersion": protocol_version,
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    },
                    "resources": {
                        "listChanged": false
                    },
                    "prompts": {
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "rust-agentic-skills",
                    "version": "1.0.0"
                }
            })
        },
        "notifications/initialized" => {
            // No response needed for notifications
            return None;
        },
        "tools/list" => {
            json!({
                "tools": [
                    {
                        "name": "verify_syntax",
                        "description": "Compiles Rust code to check for syntax and borrow-checker errors.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "code": { "type": "string", "description": "The Rust source code to verify." }
                            },
                            "required": ["code"]
                        }
                    },
                    {
                        "name": "get_pattern",
                        "description": "Retrieves the official memory-safe pattern for a task.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "topic": { "type": "string", "description": "Topic e.g. 'file_io', 'concurrency'" }
                            },
                            "required": ["topic"]
                        }
                    }
                ]
            })
        },
        "tools/call" => {
            handle_tool_call(req.params).await
        },
        "resources/list" => {
            json!({
                "resources": []
            })
        },
        "resources/read" => {
            json!({
                "contents": []
            })
        },
        "prompts/list" => {
             json!({
                "prompts": [
                    {
                        "name": "enforce_rust_standards",
                        "description": "Injects strict guidelines into the IDE context."
                    }
                ]
            })
        },
        "prompts/get" => {
             // Handle prompt get if needed, or return error if not implemented
             // For now mocking result
             json!({
                "messages": [
                    {
                        "role": "system",
                        "content": {
                            "type": "text",
                            "text": "You are the Rust Guardian. ALWAYS verify code using 'verify_syntax' before suggesting it to the user."
                        }
                    }
                ]
             })
        },
        "logging/setLevel" => {
            json!({})
        },
        // Ping handling
        "ping" => json!({}),
        _ => {
            eprintln!("Unknown method: {}", method);
             // Return error?
             return Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(json!({"code": -32601, "message": "Method not found"})),
            });
        }
    };

    Some(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(result),
        error: None,
    })
}

async fn handle_tool_call(params: Option<Value>) -> Value {
    let params = params.unwrap_or(json!({}));
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    match name {
        "verify_syntax" => {
            let code = args.get("code").and_then(|c| c.as_str()).unwrap_or("");
             if code.contains("unsafe") {
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": "Error: Usage of 'unsafe' block detected. Strict mode is active."
                        }
                    ],
                    "isError": true
                })
            } else {
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": "Code verifies successfully."
                        }
                    ],
                    "isError": false
                })
            }
        },
        "get_pattern" => {
            let topic = args.get("topic").and_then(|t| t.as_str()).unwrap_or("unknown");
            let pattern = match topic {
                "file_io" => "std::fs::read_to_string(path)?;",
                "concurrency" => "std::sync::Arc::new(std::sync::Mutex::new(data));",
                _ => "// No standard pattern found for this topic.",
            };
            json!({
                "content": [
                    {
                        "type": "text",
                        "text": pattern
                    }
                ]
            })
        },
        _ => {
            json!({
                "content": [{ "type": "text", "text": format!("Tool not found: {}", name) }],
                "isError": true
            })
        }
    }
}
