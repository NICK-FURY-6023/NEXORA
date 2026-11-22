use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

use nexora_common::ServiceName;
use nexora_error::{NexoraError, NexoraResult};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkerId(String);

impl WorkerId {
    pub fn new(value: impl Into<String>) -> NexoraResult<Self> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 {
            return Err(NexoraError::invalid_input("invalid worker id"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Decode,
    Encode,
    Resample,
    Source,
    Delivery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRegistration {
    pub id: WorkerId,
    pub cpu_capacity: u32,
    pub active_jobs: u32,
    pub capabilities: Vec<Capability>,
    pub healthy: bool,
}

impl WorkerRegistration {
    pub fn new(
        id: impl Into<String>,
        cpu_capacity: u32,
        active_jobs: u32,
        capabilities: Vec<Capability>,
    ) -> NexoraResult<Self> {
        if cpu_capacity == 0 {
            return Err(NexoraError::invalid_input(
                "worker cpu capacity must be positive",
            ));
        }
        Ok(Self {
            id: WorkerId::new(id)?,
            cpu_capacity,
            active_jobs,
            capabilities,
            healthy: true,
        })
    }

    pub fn headroom(&self) -> u32 {
        self.cpu_capacity
            .saturating_sub(self.active_jobs.saturating_mul(5))
    }
}

#[derive(Debug, Default)]
pub struct WorkerRegistry {
    workers: RwLock<HashMap<WorkerId, WorkerRegistration>>,
}

impl WorkerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, worker: WorkerRegistration) -> NexoraResult<()> {
        let mut workers = self
            .workers
            .write()
            .map_err(|_| NexoraError::internal("registry lock poisoned"))?;
        if workers.contains_key(&worker.id) {
            return Err(NexoraError::invalid_input("duplicate worker id"));
        }
        workers.insert(worker.id.clone(), worker);
        Ok(())
    }

    pub fn unregister(&self, id: &WorkerId) -> NexoraResult<()> {
        let mut workers = self
            .workers
            .write()
            .map_err(|_| NexoraError::internal("registry lock poisoned"))?;
        workers
            .remove(id)
            .ok_or_else(|| NexoraError::service("worker not found"))?;
        Ok(())
    }

    pub fn workers(&self) -> Vec<WorkerRegistration> {
        self.workers
            .read()
            .map(|workers| workers.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn with_capability(
        &self,
        capability: Capability,
    ) -> impl Iterator<Item = WorkerRegistration> {
        self.workers()
            .into_iter()
            .filter(move |worker| worker.capabilities.contains(&capability))
    }

    pub fn service_name() -> ServiceName {
        ServiceName::Cluster
    }
}

#[derive(Debug, Default)]
pub struct WeightedScheduler;

impl WeightedScheduler {
    pub fn new() -> Self {
        Self
    }

    pub fn select(
        &self,
        registry: &WorkerRegistry,
        capability: Capability,
        preferred: Option<&str>,
    ) -> NexoraResult<WorkerRegistration> {
        registry
            .with_capability(capability)
            .filter(|worker| worker.healthy)
            .max_by_key(|worker| {
                let affinity = preferred
                    .map(|value| u32::from(worker.id.as_str() == value))
                    .unwrap_or_default();
                worker.headroom() + affinity
            })
            .ok_or_else(|| NexoraError::service("no eligible worker"))
    }
}
