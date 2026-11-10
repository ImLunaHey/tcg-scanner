use crate::{
    Gateway,
    access::Principal,
    error::{Error, Result},
    gateway::header_value,
    path::Path,
};
use axum::{
    body::Body,
    http::request::Parts,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

impl Gateway {
    pub(crate) async fn event_stream(
        self: &Arc<Self>,
        head: &Parts,
        path: &Path,
        principal: Principal,
    ) -> Result<Response> {
        path.only(&["after", "seconds"])?;
        let seconds = path.number("seconds", 25, 1, 25)?;
        let mut cursor = path.number("after", 0, 0, 9_007_199_254_740_991)? as i64;
        let last = header_value(head, "last-event-id".parse().unwrap())?;
        if let Some(last) = last {
            if path.query.contains_key("after") {
                return Err(Error::params("Use Last-Event-ID or after, not both."));
            }
            cursor = last
                .parse()
                .ok()
                .filter(|v| *v >= 0 && *v <= 9_007_199_254_740_991)
                .ok_or_else(|| Error::params("Invalid event cursor."))?;
        }
        let live_only = last.is_none() && !path.query.contains_key("after");
        if live_only {
            cursor = self.engine.events(0, 100, 0).await?.latest_cursor;
        }
        let mut stream = Events {
            gateway: self.clone(),
            principal,
            cursor,
            deadline: Instant::now() + Duration::from_secs(seconds),
            pending: VecDeque::from(["retry: 1000\n\n".into()]),
            done: false,
        };
        if live_only {
            stream
                .pending
                .push_back(format!("id: {cursor}\nevent: ready\ndata: {{}}\n\n"));
        }
        stream.load(0).await?;
        let body = Body::from_stream(futures_util::stream::unfold(
            stream,
            |mut stream| async move {
                loop {
                    if let Some(frame) = stream.pending.pop_front() {
                        return Some((Ok::<_, std::io::Error>(frame), stream));
                    }
                    if stream.done || Instant::now() >= stream.deadline {
                        return None;
                    }
                    if let Err(error) = stream.load(1).await {
                        stream.done = true;
                        stream
                            .pending
                            .push_back(format!("event: error\ndata: {}\n\n", json!(error.error)));
                    }
                }
            },
        ));
        let mut response = body.into_response();
        response
            .headers_mut()
            .insert("content-type", "text/event-stream".parse().unwrap());
        response
            .headers_mut()
            .insert("x-accel-buffering", "no".parse().unwrap());
        Ok(response)
    }
}
struct Events {
    gateway: Arc<Gateway>,
    principal: Principal,
    cursor: i64,
    deadline: Instant,
    pending: VecDeque<String>,
    done: bool,
}
impl Events {
    async fn load(&mut self, wait: u32) -> Result<()> {
        if !self
            .gateway
            .access
            .lock()
            .map_err(|_| Error::internal())?
            .valid(&self.principal)
        {
            return Err(Error::new(
                401,
                "unauthorized",
                "Session expired or revoked.",
            ));
        }
        let page = self.gateway.engine.events(self.cursor, 100, wait).await?;
        if page.reset_required {
            self.pending.push_back(format!(
                "id: {}\nevent: reset\ndata: {}\n\n",
                page.latest_cursor,
                serde_json::to_string(&page).map_err(|_| Error::internal())?
            ));
            self.done = true;
            return Ok(());
        }
        for event in &page.events {
            self.pending.push_back(format!(
                "id: {}\nevent: scanner\ndata: {}\n\n",
                event.cursor,
                serde_json::to_string(event).map_err(|_| Error::internal())?
            ));
        }
        if page.events.is_empty() {
            self.pending.push_back(": heartbeat\n\n".into());
        }
        self.cursor = page.next_cursor;
        Ok(())
    }
}
