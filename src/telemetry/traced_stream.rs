use pin_project::pin_project;
use tokio_stream::Stream;

#[pin_project]
pub struct TracedStream<T: Stream> {
    #[pin]
    stream: T,
    span: tracing::Span,
}

pub trait StreamExt: Stream + Sized {
    fn trace(self, span: tracing::Span) -> TracedStream<Self> {
        TracedStream { stream: self, span }
    }
}

impl<T: Stream + Sized> StreamExt for T {}

impl<T: Stream> Stream for TracedStream<T> {
    type Item = T::Item;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.project();
        let _guard = this.span.enter();
        T::poll_next(this.stream, cx)
    }
}
