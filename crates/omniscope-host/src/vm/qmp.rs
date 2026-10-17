use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::SocketAddr;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, Lines};
use tokio::net::TcpStream;

#[derive(Error, Debug)]
pub enum QmpError {
    #[error("I/O error during QMP communication: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("QEMU QMP error response: {0}")]
    QemuError(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Handshake failed: greeting or capabilities negotiation rejected")]
    HandshakeFailed,
}

#[derive(Debug, Serialize, Deserialize)]
struct QmpCommand {
    execute: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<Value>,
}

/// Generic QEMU Machine Protocol (QMP) client over any asynchronous stream.
pub struct QmpClient<S> {
    reader: Lines<BufReader<tokio::io::ReadHalf<S>>>,
    writer: tokio::io::WriteHalf<S>,
}

impl QmpClient<TcpStream> {
    /// Connects to a QEMU QMP TCP endpoint and completes the capabilities handshake.
    pub async fn connect(addr: SocketAddr) -> Result<Self, QmpError> {
        let stream = TcpStream::connect(addr).await?;
        let mut client = Self::from_stream(stream);
        client.handshake().await?;
        Ok(client)
    }
}

impl<S> QmpClient<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    /// Creates a QMP client from an arbitrary async stream (e.g. TcpStream or duplex for testing).
    pub fn from_stream(stream: S) -> Self {
        let (read_half, write_half) = tokio::io::split(stream);
        let reader = BufReader::new(read_half).lines();
        Self {
            reader,
            writer: write_half,
        }
    }

    /// Performs the mandatory QMP initial handshake:
    /// 1. Awaits the QEMU greeting banner (`{"QMP": ...}`).
    /// 2. Sends `qmp_capabilities` command.
    /// 3. Validates the empty `{"return": {}}` acknowledgment.
    pub async fn handshake(&mut self) -> Result<(), QmpError> {
        // Step 1: Read greeting
        let greeting_line = self
            .reader
            .next_line()
            .await?
            .ok_or(QmpError::HandshakeFailed)?;

        let greeting: Value = serde_json::from_str(&greeting_line)?;
        if greeting.get("QMP").is_none() {
            return Err(QmpError::Protocol(format!(
                "Expected QMP greeting banner, received: {}",
                greeting_line
            )));
        }

        // Step 2: Send capabilities negotiation
        let resp = self.execute_command("qmp_capabilities", None).await?;
        if !resp.is_object() {
            return Err(QmpError::HandshakeFailed);
        }

        Ok(())
    }

    /// Sends a low-level QMP command and reads responses until a non-event object (`return` or `error`) is found.
    pub async fn execute_command(
        &mut self,
        execute: &str,
        arguments: Option<Value>,
    ) -> Result<Value, QmpError> {
        let cmd = QmpCommand {
            execute: execute.to_string(),
            arguments,
        };
        let mut json_str = serde_json::to_string(&cmd)?;
        json_str.push('\n');

        self.writer.write_all(json_str.as_bytes()).await?;
        self.writer.flush().await?;

        // Read lines, skipping spontaneous async event notifications
        while let Some(line) = self.reader.next_line().await? {
            let val: Value = serde_json::from_str(&line)?;

            // If it is an asynchronous event like {"event": "RESUME", ...}, ignore and keep reading
            if val.get("event").is_some() {
                continue;
            }

            if let Some(err_obj) = val.get("error") {
                let desc = err_obj
                    .get("desc")
                    .and_then(|d| d.as_str())
                    .unwrap_or("Unknown QMP error");
                return Err(QmpError::QemuError(desc.to_string()));
            }

            if let Some(ret) = val.get("return") {
                return Ok(ret.clone());
            }

            return Err(QmpError::Protocol(format!(
                "Unrecognized QMP response format: {}",
                line
            )));
        }

        Err(QmpError::Protocol(
            "Connection closed before command response received".to_string(),
        ))
    }

    /// Saves a live memory and disk state snapshot with the given identifier.
    pub async fn save_snapshot(&mut self, name: &str) -> Result<(), QmpError> {
        let cmd_line = format!("savevm {}", name);
        self.execute_command(
            "human-monitor-command",
            Some(json!({ "command-line": cmd_line })),
        )
        .await?;
        Ok(())
    }

    /// Reverts the VM memory and disk state to the specified snapshot identifier.
    pub async fn load_snapshot(&mut self, name: &str) -> Result<(), QmpError> {
        let cmd_line = format!("loadvm {}", name);
        self.execute_command(
            "human-monitor-command",
            Some(json!({ "command-line": cmd_line })),
        )
        .await?;
        Ok(())
    }

    /// Deletes a snapshot identifier from the underlying storage.
    pub async fn delete_snapshot(&mut self, name: &str) -> Result<(), QmpError> {
        let cmd_line = format!("delvm {}", name);
        self.execute_command(
            "human-monitor-command",
            Some(json!({ "command-line": cmd_line })),
        )
        .await?;
        Ok(())
    }

    /// Queries VM status ("running", "paused", "shutdown", etc.).
    pub async fn query_status(&mut self) -> Result<String, QmpError> {
        let ret = self.execute_command("query-status", None).await?;
        let status = ret
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string();
        Ok(status)
    }

    /// Pauses VM execution.
    pub async fn pause(&mut self) -> Result<(), QmpError> {
        self.execute_command("stop", None).await?;
        Ok(())
    }

    /// Resumes VM execution.
    pub async fn resume(&mut self) -> Result<(), QmpError> {
        self.execute_command("cont", None).await?;
        Ok(())
    }

    /// Instructs QEMU to immediately terminate.
    pub async fn quit(&mut self) -> Result<(), QmpError> {
        self.execute_command("quit", None).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_qmp_handshake_and_commands() {
        let (client_io, mut server_io) = tokio::io::duplex(16 * 1024);
        let mut client = QmpClient::from_stream(client_io);

        // Server simulation task
        let server_task = tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

            // 1. Send QMP greeting banner
            let greeting = "{\"QMP\":{\"version\":{\"qemu\":{\"major\":8,\"minor\":2,\"micro\":0}},\"capabilities\":[]}}\n";
            server_io.write_all(greeting.as_bytes()).await.unwrap();

            let (server_reader, mut server_writer) = tokio::io::split(server_io);
            let mut lines = BufReader::new(server_reader).lines();

            // 2. Expect qmp_capabilities command
            let cap_cmd = lines.next_line().await.unwrap().unwrap();
            assert!(cap_cmd.contains("\"execute\":\"qmp_capabilities\""));
            let cap_resp = "{\"return\":{}}\n";
            server_writer.write_all(cap_resp.as_bytes()).await.unwrap();

            // 3. Process incoming commands
            while let Some(line) = lines.next_line().await.unwrap() {
                let val: Value = serde_json::from_str(&line).unwrap();
                let exec = val["execute"].as_str().unwrap();

                match exec {
                    "human-monitor-command" => {
                        let cmd_line = val["arguments"]["command-line"].as_str().unwrap();
                        if cmd_line.starts_with("savevm ")
                            || cmd_line.starts_with("loadvm ")
                            || cmd_line.starts_with("delvm ")
                        {
                            let resp = "{\"return\":\"\"}\n";
                            server_writer.write_all(resp.as_bytes()).await.unwrap();
                        } else {
                            let err_resp = "{\"error\":{\"class\":\"GenericError\",\"desc\":\"Invalid monitor command\"}}\n";
                            server_writer.write_all(err_resp.as_bytes()).await.unwrap();
                        }
                    }
                    "query-status" => {
                        // Emit a spontaneous async event first to test event filtering!
                        let event = "{\"event\":\"RESUME\",\"timestamp\":{\"seconds\":1710000000,\"microseconds\":0}}\n";
                        server_writer.write_all(event.as_bytes()).await.unwrap();

                        let status_resp = "{\"return\":{\"running\":true,\"singlestep\":false,\"status\":\"running\"}}\n";
                        server_writer.write_all(status_resp.as_bytes()).await.unwrap();
                    }
                    "quit" => {
                        let resp = "{\"return\":{}}\n";
                        server_writer.write_all(resp.as_bytes()).await.unwrap();
                        break;
                    }
                    _ => {
                        let err_resp = "{\"error\":{\"class\":\"CommandNotFound\",\"desc\":\"The command has not been found\"}}\n";
                        server_writer.write_all(err_resp.as_bytes()).await.unwrap();
                    }
                }
            }
        });

        // 1. Handshake
        client.handshake().await.expect("Handshake must succeed");

        // 2. Save snapshot
        client
            .save_snapshot("clean_boot")
            .await
            .expect("savevm must succeed");

        // 3. Query status (verifies spontaneous event is skipped)
        let status = client.query_status().await.expect("query-status must succeed");
        assert_eq!(status, "running");

        // 4. Load snapshot
        client
            .load_snapshot("clean_boot")
            .await
            .expect("loadvm must succeed");

        // 5. Delete snapshot
        client
            .delete_snapshot("clean_boot")
            .await
            .expect("delvm must succeed");

        // 6. Quit
        client.quit().await.expect("quit must succeed");

        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_qmp_error_handling() {
        let (client_io, mut server_io) = tokio::io::duplex(16 * 1024);
        let mut client = QmpClient::from_stream(client_io);

        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            let greeting = "{\"QMP\":{\"version\":{\"qemu\":{\"major\":8,\"minor\":2,\"micro\":0}},\"capabilities\":[]}}\n";
            server_io.write_all(greeting.as_bytes()).await.unwrap();

            let (server_reader, mut server_writer) = tokio::io::split(server_io);
            let mut lines = BufReader::new(server_reader).lines();

            let _cap = lines.next_line().await.unwrap();
            server_writer.write_all(b"{\"return\":{}}\n").await.unwrap();

            let _cmd = lines.next_line().await.unwrap();
            let err_resp = "{\"error\":{\"class\":\"DeviceNotFound\",\"desc\":\"Device 'invalid_device' not found\"}}\n";
            server_writer.write_all(err_resp.as_bytes()).await.unwrap();
        });

        client.handshake().await.unwrap();
        let result = client.execute_command("nonexistent_command", None).await;
        match result {
            Err(QmpError::QemuError(desc)) => {
                assert!(desc.contains("Device 'invalid_device' not found"));
            }
            other => panic!("Expected QemuError, got {:?}", other),
        }
    }
}
