use std::{fs::File, io::Write, sync::Arc};

use locutus_core::{ContractExecutor, SqlitePool};
use locutus_runtime::{ContractStore, StateStore};
use tokio::sync::RwLock;

use crate::{
    config::{DeserializationFmt, LocalNodeCliConfig},
    DynError,
};

#[derive(Clone)]
pub(super) struct AppState {
    pub(crate) local_node: Arc<RwLock<ContractExecutor>>,
    config: LocalNodeCliConfig,
}

impl AppState {
    const MAX_MEM_CACHE: u32 = 10_000_000;

    pub async fn new(config: &LocalNodeCliConfig) -> Result<Self, DynError> {
        let tmp_path = std::env::temp_dir().join("locutus").join("contracts");
        std::fs::create_dir_all(&tmp_path)?;
        let contract_store =
            ContractStore::new(tmp_path.join("contracts"), config.max_contract_size);
        let state_store = StateStore::new(SqlitePool::new().await?, Self::MAX_MEM_CACHE).unwrap();
        Ok(AppState {
            local_node: Arc::new(RwLock::new(
                ContractExecutor::new(contract_store, state_store, || {
                    locutus_core::util::set_cleanup_on_exit().unwrap();
                })
                .await?,
            )),
            config: config.clone(),
        })
    }

    pub fn printout_deser<R: AsRef<[u8]> + ?Sized>(&self, data: &R) -> Result<(), std::io::Error> {
        fn write_res(config: &LocalNodeCliConfig, pprinted: &str) -> Result<(), std::io::Error> {
            if let Some(p) = &config.output_file {
                let mut f = File::create(p)?;
                f.write_all(pprinted.as_bytes())?;
            } else if config.terminal_output {
                tracing::debug!("{pprinted}");
            }
            Ok(())
        }
        match self.config.ser_format {
            Some(DeserializationFmt::Json) => {
                let deser: serde_json::Value = serde_json::from_slice(data.as_ref())?;
                let pp = serde_json::to_string_pretty(&deser)?;
                write_res(&self.config, &*pp)?;
            }
            #[cfg(feature = "messagepack")]
            Some(DeserializationFmt::MessagePack) => {
                let deser = rmpv::decode::read_value(&mut data.as_ref())
                    .map_err(|_err| std::io::ErrorKind::InvalidData)?;
                let pp = format!("{deser}");
                write_res(&self.config, &*pp)?;
            }
            _ => {}
        }
        Ok(())
    }
}
