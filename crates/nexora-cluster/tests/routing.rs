use nexora_cluster::{Capability, ClusterRouter, WorkerId, WorkerRegistration, WorkerRegistry};

fn worker(id: &str, cpu: u32, jobs: u32, capabilities: &[Capability]) -> WorkerRegistration {
    WorkerRegistration::new(id, cpu, jobs, capabilities.to_vec()).unwrap()
}

#[test]
fn a_session_is_routed_to_a_worker_that_has_the_capability() {
    let registry = WorkerRegistry::new();
    registry
        .register(worker("w1", 100, 0, &[Capability::Decode]))
        .unwrap();
    registry
        .register(worker("w2", 100, 0, &[Capability::Encode]))
        .unwrap();

    let mut router = ClusterRouter::new();
    let id = router
        .route(&registry, "session-1", Capability::Decode)
        .expect("a decoder should be found");
    assert_eq!(id.as_str(), "w1");
}

#[test]
fn a_session_stays_on_the_worker_it_was_routed_to() {
    let registry = WorkerRegistry::new();
    registry
        .register(worker("w1", 100, 0, &[Capability::Decode]))
        .unwrap();
    registry
        .register(worker("w2", 100, 0, &[Capability::Decode]))
        .unwrap();

    let mut router = ClusterRouter::new();
    let first = router
        .route(&registry, "session-1", Capability::Decode)
        .expect("a decoder should be found");
    let second = router
        .route(&registry, "session-1", Capability::Decode)
        .expect("the same session should be routed again");
    assert_eq!(first, second);
}

#[test]
fn a_session_is_routed_again_when_its_worker_goes_away() {
    let registry = WorkerRegistry::new();
    let w1 = worker("w1", 100, 0, &[Capability::Decode]);
    registry.register(w1.clone()).unwrap();
    registry
        .register(worker("w2", 100, 0, &[Capability::Decode]))
        .unwrap();

    let mut router = ClusterRouter::new();
    let first = router
        .route(&registry, "session-1", Capability::Decode)
        .expect("a decoder should be found");
    assert_eq!(first.as_str(), "w1");

    registry.unregister(&w1.id).unwrap();
    let second = router
        .route(&registry, "session-1", Capability::Decode)
        .expect("the session should move to the other worker");
    assert_eq!(second.as_str(), "w2");
}

#[test]
fn a_session_with_no_eligible_worker_is_refused() {
    let registry = WorkerRegistry::new();
    let mut router = ClusterRouter::new();
    let error = router
        .route(&registry, "session-1", Capability::Decode)
        .expect_err("there is no worker to take the session");
    assert_eq!(error.code(), nexora_error::ErrorCode::Service);
}

#[test]
fn a_worker_that_registers_twice_is_refused() {
    let registry = WorkerRegistry::new();
    registry
        .register(worker("w1", 100, 0, &[Capability::Decode]))
        .unwrap();
    let error = registry
        .register(worker("w1", 100, 0, &[Capability::Decode]))
        .expect_err("the same worker must not register twice");
    assert_eq!(error.code(), nexora_error::ErrorCode::InvalidInput);
}

#[test]
fn a_worker_reports_its_load_and_its_health() {
    let registry = WorkerRegistry::new();
    let registration = worker("w1", 100, 0, &[Capability::Decode]);
    registry.register(registration.clone()).unwrap();

    registry
        .update_load(&registration.id, 40)
        .expect("the load should be recorded");
    let workers = registry.workers();
    assert_eq!(workers.len(), 1);
    assert_eq!(workers[0].active_jobs, 40);

    registry
        .set_healthy(&registration.id, false)
        .expect("the health should be recorded");
    let workers = registry.workers();
    assert!(!workers[0].healthy);
}

#[test]
fn an_unhealthy_worker_is_not_offered_a_session() {
    let registry = WorkerRegistry::new();
    let registration = worker("w1", 100, 0, &[Capability::Decode]);
    registry.register(registration.clone()).unwrap();
    registry.set_healthy(&registration.id, false).unwrap();

    let mut router = ClusterRouter::new();
    let error = router
        .route(&registry, "session-1", Capability::Decode)
        .expect_err("an unhealthy worker must not take the session");
    assert_eq!(error.code(), nexora_error::ErrorCode::Service);
}

#[test]
fn a_session_can_be_released() {
    let registry = WorkerRegistry::new();
    registry
        .register(worker("w1", 100, 0, &[Capability::Decode]))
        .unwrap();
    let mut router = ClusterRouter::new();
    router
        .route(&registry, "session-1", Capability::Decode)
        .expect("a decoder should be found");
    router.release("session-1");
    assert!(router.worker_for("session-1").is_none());
}

#[test]
fn the_router_names_the_worker_a_session_is_on() {
    let registry = WorkerRegistry::new();
    registry
        .register(worker("w1", 100, 0, &[Capability::Decode]))
        .unwrap();
    let mut router = ClusterRouter::new();
    let id = router
        .route(&registry, "session-1", Capability::Decode)
        .expect("a decoder should be found");
    assert_eq!(router.worker_for("session-1"), Some(&id));
}

#[test]
fn a_worker_id_is_validated() {
    assert!(WorkerId::new("").is_err());
    assert!(WorkerId::new("worker-1").is_ok());
}
