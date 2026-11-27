use nexora_cluster::{Capability, ClusterRouter, WorkerId, WorkerRegistration, WorkerRegistry};

fn worker(id: &str) -> WorkerRegistration {
    WorkerRegistration::new(id, 100, 0, vec![Capability::Decode]).unwrap()
}

#[test]
fn routing_avoids_a_worker_that_was_marked_down() {
    let registry = WorkerRegistry::new();
    let w1 = worker("w1");
    registry.register(w1.clone()).unwrap();
    registry.register(worker("w2")).unwrap();

    let mut router = ClusterRouter::new();
    let first = router
        .route(&registry, "session-1", Capability::Decode)
        .unwrap();
    assert_eq!(first.as_str(), "w1");

    registry.mark_down(&w1.id).unwrap();
    let second = router
        .route(&registry, "session-1", Capability::Decode)
        .unwrap();
    assert_eq!(second.as_str(), "w2");
}

#[test]
fn routing_fails_when_every_worker_is_down() {
    let registry = WorkerRegistry::new();
    let w1 = worker("w1");
    registry.register(w1.clone()).unwrap();

    registry.mark_down(&w1.id).unwrap();
    let mut router = ClusterRouter::new();
    assert!(
        router
            .route(&registry, "session-1", Capability::Decode)
            .is_err()
    );
}

#[test]
fn an_unknown_worker_cannot_be_marked_down() {
    let registry = WorkerRegistry::new();
    let id = WorkerId::new("missing").unwrap();
    assert!(registry.mark_down(&id).is_err());
}
