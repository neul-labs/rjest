use anyhow::{Context, Result};
use nng::options::{Options, RecvTimeout, SendTimeout};
use nng::{Protocol, Socket};
use rjest_protocol::{ipc_address, Request, Response};
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Send a request to the daemon and wait for response
pub fn send_request(request: Request) -> Result<Response> {
    let socket = Socket::new(Protocol::Req0).context("Failed to create socket")?;

    // Set timeouts
    socket
        .set_opt::<SendTimeout>(Some(DEFAULT_TIMEOUT))
        .context("Failed to set send timeout")?;
    socket
        .set_opt::<RecvTimeout>(Some(DEFAULT_TIMEOUT))
        .context("Failed to set recv timeout")?;

    // Connect to daemon
    let addr = ipc_address();
    socket
        .dial(&addr)
        .context("Failed to connect to daemon. Is it running?")?;

    // Serialize and send request
    let request_bytes = serde_json::to_vec(&request).context("Failed to serialize request")?;
    socket
        .send(&request_bytes)
        .map_err(|(_, e)| e)
        .context("Failed to send request")?;

    // Receive response
    let response_bytes = socket.recv().context("Failed to receive response")?;
    let response: Response =
        serde_json::from_slice(&response_bytes).context("Failed to parse response")?;

    Ok(response)
}

/// Check if daemon is reachable
pub fn ping() -> Result<bool> {
    let socket = Socket::new(Protocol::Req0)?;
    socket.set_opt::<SendTimeout>(Some(Duration::from_secs(2)))?;
    socket.set_opt::<RecvTimeout>(Some(Duration::from_secs(2)))?;

    let addr = ipc_address();
    if socket.dial(&addr).is_err() {
        return Ok(false);
    }

    let request_bytes = serde_json::to_vec(&Request::Ping)?;
    if socket.send(&request_bytes).is_err() {
        return Ok(false);
    }

    match socket.recv() {
        Ok(bytes) => {
            let response: Response = serde_json::from_slice(&bytes)?;
            Ok(matches!(response, Response::Pong))
        }
        Err(_) => Ok(false),
    }
}
