use axum::body::{Body, Bytes};
use http_body::{Body as HttpBody, Frame, SizeHint};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::OwnedSemaphorePermit;

pub(crate) struct LeaseBody {
    body: Pin<Box<Body>>,
    _permits: Vec<OwnedSemaphorePermit>,
}
impl LeaseBody {
    pub fn wrap(body: Body, permits: Vec<OwnedSemaphorePermit>) -> Body {
        Body::new(Self {
            body: Box::pin(body),
            _permits: permits,
        })
    }
}
impl HttpBody for LeaseBody {
    type Data = Bytes;
    type Error = axum::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<std::result::Result<Frame<Bytes>, Self::Error>>> {
        self.get_mut().body.as_mut().poll_frame(cx)
    }
    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}
