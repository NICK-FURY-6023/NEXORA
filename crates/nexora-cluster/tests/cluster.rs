use nexora_cluster::{Capability, WeightedScheduler, WorkerRegistration, WorkerRegistry};

fn worker(id: &str, cpu: u32, jobs: u32, capabilities: &[Capability]) -> WorkerRegistration {
    WorkerRegistration::new(id, cpu, jobs, capabilities.to_vec()).unwrap()
}

#[test]
fn registry_registers_and_filters_workers() {
    let registry = WorkerRegistry::new();
    registry
        .register(worker("w1", 100, 0, &[Capability::Decode]))
        .unwrap();
    registry
        .register(worker("w2", 50, 10, &[Capability::Encode]))
        .unwrap();
    assert_eq!(registry.workers().len(), 2);
    let decoders: Vec<_> = registry.with_capability(Capability::Decode).collect();
    assert_eq!(decoders.len(), 1);
    assert_eq!(decoders[0].id.as_str(), "w1");
}

#[test]
fn scheduler_prefers_headroom_and_capability() {
    let registry = WorkerRegistry::new();
    registry
        .register(worker("busy", 100, 90, &[Capability::Decode]))
        .unwrap();
    registry
        .register(worker("free", 80, 0, &[Capability::Decode]))
        .unwrap();
    let scheduler = WeightedScheduler::new();
    let selected = scheduler
        .select(&registry, Capability::Decode, Some("busy"))
        .unwrap();
    assert_eq!(selected.id.as_str(), "free");
}
