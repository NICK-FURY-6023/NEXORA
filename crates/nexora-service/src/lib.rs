use nexora_common::ServiceName;
use nexora_error::NexoraResult;
use tokio::sync::watch;

#[derive(Debug, Clone)]
pub struct Shutdown {
    sender: watch::Sender<bool>,
}

#[derive(Debug, Clone)]
pub struct ShutdownReceiver {
    receiver: watch::Receiver<bool>,
}

impl Shutdown {
    pub fn new() -> (Self, ShutdownReceiver) {
        let (sender, receiver) = watch::channel(false);
        (Self { sender }, ShutdownReceiver { receiver })
    }

    pub fn trigger(&self) {
        let _ = self.sender.send(true);
    }

    pub fn is_triggered(&self) -> bool {
        *self.sender.borrow()
    }

    pub fn receiver(&self) -> ShutdownReceiver {
        ShutdownReceiver {
            receiver: self.sender.subscribe(),
        }
    }
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new().0
    }
}

impl ShutdownReceiver {
    pub async fn wait(&mut self) {
        while !*self.receiver.borrow_and_update() {
            if self.receiver.changed().await.is_err() {
                break;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthReport {
    pub service: ServiceName,
    pub healthy: bool,
    pub message: String,
}

impl HealthReport {
    pub fn healthy(service: ServiceName, message: impl Into<String>) -> Self {
        Self {
            service,
            healthy: true,
            message: message.into(),
        }
    }

    pub fn unhealthy(service: ServiceName, message: impl Into<String>) -> Self {
        Self {
            service,
            healthy: false,
            message: message.into(),
        }
    }
}

// Phase 1 service trait is used only within this workspace; auto-trait bounds
// on the returned future will be added when services become independently deployed.
#[allow(async_fn_in_trait)]
pub trait Service {
    fn name(&self) -> ServiceName;
    fn health(&self) -> HealthReport;
    async fn run(&self, shutdown: ShutdownReceiver) -> NexoraResult<()>;
}

pub async fn run_until_shutdown<S: Service>(
    service: S,
    shutdown: ShutdownReceiver,
) -> NexoraResult<()> {
    service.run(shutdown).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_trigger_is_observable() {
        let (shutdown, receiver) = Shutdown::new();
        assert!(!shutdown.is_triggered());
        assert!(!*receiver.receiver.borrow());
        shutdown.trigger();
        assert!(shutdown.is_triggered());
        assert!(*receiver.receiver.borrow());
    }

    #[tokio::test]
    async fn shutdown_receiver_wakes() {
        let (shutdown, mut receiver) = Shutdown::new();
        let other = shutdown.clone();
        tokio::spawn(async move {
            tokio::task::yield_now().await;
            other.trigger();
        });
        receiver.wait().await;
        assert!(shutdown.is_triggered());
    }

    #[test]
    fn health_report_constructors() {
        let healthy = HealthReport::healthy(ServiceName::Gateway, "ok");
        assert!(healthy.healthy);
        let unhealthy = HealthReport::unhealthy(ServiceName::Session, "down");
        assert!(!unhealthy.healthy);
    }
}
