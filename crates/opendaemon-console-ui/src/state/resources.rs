use opendaemon_console_api::dto::{
    AgentProfile, DaemonStatus, DirectoryGrant, PermissionRequest, Product, Provider, RuntimeView,
};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ResourceState {
    pub status: Option<DaemonStatus>,
    pub products: Vec<Product>,
    pub providers: Vec<Provider>,
    pub runtimes: Vec<RuntimeView>,
    pub agents: Vec<AgentProfile>,
    pub directories: Vec<DirectoryGrant>,
    pub permissions: Vec<PermissionRequest>,
    pub loading: bool,
    pub error: Option<String>,
}
