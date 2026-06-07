use std::{collections::BTreeMap, sync::Arc};

use tokio::sync::RwLock;

use crate::registry::{IntegrationType, ProviderManifest};

use super::model::{RuntimeKind, RuntimeView};

#[derive(Debug, Clone, Default)]
pub struct RuntimeStore {
    runtimes: Arc<RwLock<BTreeMap<String, RuntimeView>>>,
}

impl RuntimeStore {
    pub async fn save(&self, runtime: RuntimeView) {
        self.runtimes
            .write()
            .await
            .insert(runtime.provider_id.clone(), runtime);
    }

    pub async fn save_all(&self, runtimes: impl IntoIterator<Item = RuntimeView>) {
        let mut stored = self.runtimes.write().await;

        for runtime in runtimes {
            stored.insert(runtime.provider_id.clone(), runtime);
        }
    }

    pub async fn get(&self, provider_id: &str) -> Option<RuntimeView> {
        self.runtimes.read().await.get(provider_id).cloned()
    }

    pub async fn snapshot(&self) -> Vec<RuntimeView> {
        let mut runtimes = self
            .runtimes
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        runtimes.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        runtimes
    }

    pub async fn list_for_providers(&self, providers: &[ProviderManifest]) -> Vec<RuntimeView> {
        let stored = self.runtimes.read().await;
        let mut runtimes = providers
            .iter()
            .filter(|provider| {
                matches!(
                    provider.integration_type,
                    IntegrationType::Cli | IntegrationType::Acp | IntegrationType::Http
                )
            })
            .map(|provider| {
                stored.get(&provider.id).cloned().unwrap_or_else(|| {
                    RuntimeView::not_detected_with_kind(
                        provider.id.clone(),
                        match provider.integration_type {
                            IntegrationType::Cli => RuntimeKind::LocalCli,
                            IntegrationType::Acp => RuntimeKind::LocalAcp,
                            IntegrationType::Http => RuntimeKind::RemoteHttp,
                            IntegrationType::Native => RuntimeKind::LocalCli,
                        },
                    )
                })
            })
            .collect::<Vec<_>>();

        runtimes.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        runtimes
    }
}
