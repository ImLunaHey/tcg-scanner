use crate::{
    Service, catalogue,
    framing::{self, Frame},
    protocol::{Action, Protocol, failure, success},
    tool::{self, Tool},
};
use serde_json::{Value, json};
use std::{collections::HashMap, io, sync::Arc};
use tokio::{
    io::{AsyncRead, AsyncWrite, BufReader},
    sync::mpsc,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

pub struct Server {
    service: Arc<dyn Service>,
    tools: Vec<Tool>,
}
impl Server {
    pub fn new(service: Arc<dyn Service>, read_only: bool) -> Self {
        Self {
            service,
            tools: catalogue::tools()
                .into_iter()
                .filter(|tool| !read_only || !tool.changes_state)
                .collect(),
        }
    }
    /// Serve one MCP session. No scanner operation is performed before a tool
    /// call; reads, output buffering and active calls are bounded independently.
    pub async fn serve<R, W>(&self, input: R, mut output: W) -> io::Result<()>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (sender, mut receiver) = mpsc::channel(8);
        let mut writer = tokio::spawn(async move {
            while let Some(value) = receiver.recv().await {
                framing::write(&mut output, value).await?;
            }
            Ok::<_, io::Error>(())
        });
        let mut reader = BufReader::new(input);
        let mut frame = Frame::default();
        let mut protocol = Protocol::default();
        let mut active = HashMap::new();
        let mut tasks = JoinSet::new();
        let result = loop {
            tokio::select! {
                result=&mut writer=>{tasks.abort_all();return result.map_err(io::Error::other)?;},
                Some(result)=tasks.join_next(),if !tasks.is_empty()=>{match result {Ok(id)=>{active.remove(&id);},Err(error)=>break Err(io::Error::other(error))}},
                message=frame.read(&mut reader)=>{
                    let bytes=match message {Ok(Some(bytes))=>bytes,Ok(None)=>break Ok(()),Err(error)=>break Err(error)};
                    let reply=match protocol.receive(&bytes,&self.tools) {
                        Action::Reply(value)=>Some(value),
                        Action::Ignore=>None,
                        Action::Cancel(id)=>{if let Some(token)=active.get(&id.to_string()){CancellationToken::cancel(token);}None},
                        Action::Call(request)=>self.start(request,&sender,&mut active,&mut tasks),
                    };
                    if let Some(reply)=reply && sender.send(reply).await.is_err(){break Err(io::Error::new(io::ErrorKind::BrokenPipe,"MCP output closed."));}
                }
            }
        };
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        drop(sender);
        let written = writer.await.map_err(io::Error::other)?;
        result.and(written)
    }
    fn start(
        &self,
        request: Value,
        sender: &mpsc::Sender<Value>,
        active: &mut HashMap<String, CancellationToken>,
        tasks: &mut JoinSet<String>,
    ) -> Option<Value> {
        let id = request["id"].clone();
        let key = id.to_string();
        if active.contains_key(&key) {
            return Some(failure(id, -32600, "Request ID is already active."));
        }
        if active.len() >= 8 {
            return Some(failure(
                id,
                -32000,
                "At most eight tool calls may be active.",
            ));
        }
        let params = &request["params"];
        if params.as_object().is_none_or(|p| {
            p.keys()
                .any(|k| !matches!(k.as_str(), "name" | "arguments" | "_meta"))
        }) {
            return Some(failure(id, -32602, "Invalid tool call parameters."));
        }
        let Some(tool) = self
            .tools
            .iter()
            .find(|tool| Some(tool.name) == params["name"].as_str())
        else {
            return Some(failure(id, -32602, "Unknown or disabled tool."));
        };
        let arguments = match tool.parameters(params.get("arguments").cloned().unwrap_or(json!({})))
        {
            Ok(value) => value,
            Err(error) => return Some(success(id, tool::result(Err(error)))),
        };
        let token = CancellationToken::new();
        active.insert(key.clone(), token.clone());
        let service = self.service.clone();
        let method = tool.method;
        let output = sender.clone();
        tasks.spawn(async move {
            tokio::select! {
                biased;
                _=token.cancelled()=>{},
                result=service.call(method,arguments)=>{let _=output.send(success(id,tool::result(result))).await;},
            }
            key
        });
        None
    }
}
