use super::Incoming;
use super::service::DumpFn;
use zbus::fdo;

pub(crate) struct TestIface {
    tx: async_channel::Sender<Incoming>,
    dump: DumpFn,
}

impl TestIface {
    pub(crate) fn new(tx: async_channel::Sender<Incoming>, dump: DumpFn) -> Self {
        TestIface { tx, dump }
    }

    async fn forward(&self, event: Incoming) -> fdo::Result<()> {
        self.tx
            .send(event)
            .await
            .map_err(|_| fdo::Error::Failed("the app is shutting down".into()))
    }
}

#[zbus::interface(name = "dev.easyshot.app.Test")]
impl TestIface {
    async fn inject(&self, events: String) -> fdo::Result<()> {
        self.forward(Incoming::Inject(events)).await
    }

    async fn set_pointer(&self, x: f64, y: f64) -> fdo::Result<()> {
        self.forward(Incoming::SetPointer { x, y }).await
    }

    async fn dump(&self) -> String {
        (self.dump)().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_name_matches_the_protocol_constant() {
        use zbus::object_server::Interface;
        assert_eq!(TestIface::name(), es_protocol::TEST_INTERFACE);
    }
}
