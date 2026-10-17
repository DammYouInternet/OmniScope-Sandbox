use crate::ipc::error::IpcError;
use futures_util::{SinkExt, StreamExt};
use omniscope_core::commands::{GuestCommand, GuestResponse};
use omniscope_core::protocol::Frame;
use omniscope_core::ProtocolCodec;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot};
use tokio_util::codec::Framed;

/// High-level asynchronous IPC client running on the Host, communicating with the Guest Agent.
#[derive(Clone)]
pub struct HostIpcClient {
    seq_counter: Arc<AtomicU64>,
    cmd_tx: mpsc::Sender<(Frame, oneshot::Sender<GuestResponse>)>,
}

impl HostIpcClient {
    /// Connects the IPC client to an asynchronous bidirectional stream (VSOCK, VirtIO, or duplex pipe).
    pub fn new<S>(stream: S) -> Self
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let seq_counter = Arc::new(AtomicU64::new(1));
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<(Frame, oneshot::Sender<GuestResponse>)>(64);

        let framed = Framed::new(stream, ProtocolCodec::new());
        let (mut sink, mut stream) = framed.split();

        tokio::spawn(async move {
            let mut in_flight: HashMap<u64, oneshot::Sender<GuestResponse>> = HashMap::new();

            loop {
                tokio::select! {
                    // Outgoing commands to be framed and sent to guest
                    Some((frame, response_tx)) = cmd_rx.recv() => {
                        let seq = frame.header.sequence_id;
                        in_flight.insert(seq, response_tx);
                        if let Err(e) = sink.send(frame).await {
                            tracing::error!("Failed to send frame over IPC sink: {}", e);
                            break;
                        }
                    }

                    // Incoming response frames from guest
                    maybe_frame = stream.next() => {
                        match maybe_frame {
                            Some(Ok(frame)) => {
                                let seq = frame.header.sequence_id;
                                if let Some(tx) = in_flight.remove(&seq) {
                                    match frame.decode_response() {
                                        Ok(resp) => { let _ = tx.send(resp); }
                                        Err(e) => {
                                            tracing::error!("Failed to decode response payload for seq {}: {}", seq, e);
                                        }
                                    }
                                } else {
                                    tracing::warn!("Received response for untracked sequence ID: {}", seq);
                                }
                            }
                            Some(Err(e)) => {
                                tracing::error!("IPC frame decoding error: {}", e);
                                break;
                            }
                            None => {
                                tracing::info!("IPC stream closed by remote guest");
                                break;
                            }
                        }
                    }
                }
            }
        });

        Self {
            seq_counter,
            cmd_tx,
        }
    }

    /// Dispatches a command to the guest and awaits the correlated response.
    pub async fn send_command(
        &self,
        command: GuestCommand,
        trace_id: u64,
        timeout: Duration,
    ) -> Result<GuestResponse, IpcError> {
        let seq_id = self.seq_counter.fetch_add(1, Ordering::SeqCst);
        let frame = Frame::new_command(seq_id, trace_id, &command)?;

        let (resp_tx, resp_rx) = oneshot::channel();

        self.cmd_tx
            .send((frame, resp_tx))
            .await
            .map_err(|_| IpcError::Disconnected)?;

        let resp = tokio::time::timeout(timeout, resp_rx)
            .await
            .map_err(|_| IpcError::Timeout(timeout))?
            .map_err(|_| IpcError::Disconnected)?;

        if let GuestResponse::Error { code, message } = &resp {
            return Err(IpcError::GuestError {
                code: *code,
                message: message.clone(),
            });
        }

        Ok(resp)
    }
}
