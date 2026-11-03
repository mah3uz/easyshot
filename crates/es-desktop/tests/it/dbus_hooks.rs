#![cfg(feature = "test-hooks")]

use crate::dbus_support;

use dbus_support::{PrivateBus, callbacks, run};
use es_desktop::dbus::{Endpoint, Incoming, Service};
use es_protocol::{BUS_NAME_TEST, OBJECT_PATH, TEST_INTERFACE};

#[test]
fn the_test_interface_forwards_events_on_the_test_name() {
    run(async {
        let bus = PrivateBus::start();
        let svc = Service::start(Endpoint::test().on_address(&bus.address), callbacks())
            .await
            .expect("service");
        let incoming = svc.incoming();
        let conn = zbus::connection::Builder::address(bus.address.as_str())
            .expect("address")
            .build()
            .await
            .expect("connect");

        conn.call_method(
            Some(BUS_NAME_TEST),
            OBJECT_PATH,
            Some(TEST_INTERFACE),
            "Inject",
            &("key:Escape",),
        )
        .await
        .expect("inject");
        assert!(matches!(
            incoming.recv().await.expect("event"),
            Incoming::Inject(s) if s == "key:Escape"
        ));

        conn.call_method(
            Some(BUS_NAME_TEST),
            OBJECT_PATH,
            Some(TEST_INTERFACE),
            "SetPointer",
            &(1.5f64, 2.5f64),
        )
        .await
        .expect("pointer");
        assert!(matches!(
            incoming.recv().await.expect("event"),
            Incoming::SetPointer { x, y } if x == 1.5 && y == 2.5
        ));

        let dump: String = conn
            .call_method(
                Some(BUS_NAME_TEST),
                OBJECT_PATH,
                Some(TEST_INTERFACE),
                "Dump",
                &(),
            )
            .await
            .expect("dump")
            .body()
            .deserialize()
            .expect("string");
        assert_eq!(dump, "state");
    });
}
