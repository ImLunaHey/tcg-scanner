use serde_json::Value;
use std::io;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Default)]
pub(crate) struct Frame {
    bytes: Vec<u8>,
}
impl Frame {
    /// The partial buffer lives across cancellation of this future by select.
    pub async fn read(
        &mut self,
        input: &mut (impl AsyncBufRead + Unpin),
    ) -> io::Result<Option<Vec<u8>>> {
        loop {
            let available = input.fill_buf().await?;
            if available.is_empty() {
                return if self.bytes.is_empty() {
                    Ok(None)
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Incomplete MCP frame.",
                    ))
                };
            }
            let end = available.iter().position(|b| *b == b'\n');
            let count = end.map_or(available.len(), |p| p + 1);
            if self.bytes.len() + count > 65536 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "MCP frame exceeds 64 KiB.",
                ));
            }
            self.bytes.extend_from_slice(&available[..count]);
            input.consume(count);
            if end.is_some() {
                return Ok(Some(std::mem::take(&mut self.bytes)));
            }
        }
    }
}
pub(crate) async fn write(output: &mut (impl AsyncWrite + Unpin), value: Value) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(&value)?;
    if bytes.len() > 34 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "MCP response too large.",
        ));
    }
    bytes.push(b'\n');
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        output.write_all(&bytes).await?;
        output.flush().await
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "MCP output stalled."))?
}
