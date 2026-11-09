use crate::config::TitanConfig;
use crate::engine::{
    GetOptions, PutItem, PutOptions, TitanEngine, TtlItem, TtlOptions,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::Arc;

#[napi(object)]
pub struct JsPutOptions {
    pub mode: Option<String>,
    pub ttl: Option<i64>,
    pub ttl_diff: Option<i64>,
    pub sync: Option<bool>,
}

#[napi(object)]
pub struct JsPutItem {
    pub key: String,
    pub value: Buffer,
    pub options: Option<JsPutOptions>,
}

#[napi(object)]
pub struct JsGetOptions {
    pub allow_expired: Option<bool>,
    pub with_ttl: Option<bool>,
    pub with_expire_at: Option<bool>,
}

#[napi(object)]
pub struct JsGetResult {
    pub value: Buffer,
    pub ttl: i64,
    pub expire_at: i64,
}

#[napi(object)]
pub struct JsTtlOptions {
    pub ttl: Option<i64>,
    pub increase: Option<i64>,
    pub reduce: Option<i64>,
}

#[napi(object)]
pub struct JsTtlItem {
    pub key: String,
    pub ttl: Option<i64>,
    pub increase: Option<i64>,
    pub reduce: Option<i64>,
}

#[napi(object)]
pub struct JsTtlResult {
    pub ttl: i64,
    pub expire_at: i64,
}

#[napi(object)]
pub struct JsBatchResult {
    pub key: String,
    pub success: bool,
    pub error: Option<String>,
}

#[napi(object)]
pub struct JsEngineStats {
    pub total_keys: u32,
    pub segments_count: u32,
    pub live_bytes: i64,
    pub memory_used_bytes: i64,
    pub ttl_cleaned_count: i64,
    pub gc_cleaned_segments: i64,
    pub gc_reclaimed_bytes: i64,
}

#[napi]
pub struct TitanKvNative {
    engine: Arc<TitanEngine>,
}

#[napi]
impl TitanKvNative {
    #[napi(constructor)]
    pub fn new() -> Result<Self> {
        Err(Error::new(
            Status::GenericFailure,
            "Use open(config) to initialize TitanKvNative",
        ))
    }

    #[napi]
    pub async fn put(
        &self,
        key: String,
        buffer: Buffer,
        options: Option<JsPutOptions>,
    ) -> Result<bool> {
        let engine = Arc::clone(&self.engine);
        let opts = options.map(|o| PutOptions {
            mode: o.mode,
            ttl: o.ttl,
            ttl_diff: o.ttl_diff,
            sync: o.sync,
        });

        tokio::task::spawn_blocking(move || {
            engine
                .put(&key, &buffer, opts.as_ref())
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    #[napi]
    pub async fn put_many(
        &self,
        items: Vec<JsPutItem>,
        options: Option<JsPutOptions>,
    ) -> Result<Vec<JsBatchResult>> {
        let engine = Arc::clone(&self.engine);
        let default_opts = options.map(|o| PutOptions {
            mode: o.mode,
            ttl: o.ttl,
            ttl_diff: o.ttl_diff,
            sync: o.sync,
        });

        tokio::task::spawn_blocking(move || {
            let put_items: Vec<PutItem> = items
                .into_iter()
                .map(|item| PutItem {
                    key: item.key,
                    value: item.value.to_vec(),
                    options: item.options.map(|o| PutOptions {
                        mode: o.mode,
                        ttl: o.ttl,
                        ttl_diff: o.ttl_diff,
                        sync: o.sync,
                    }),
                })
                .collect();

            let results = engine
                .put_many(&put_items, default_opts.as_ref())
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

            Ok(results
                .into_iter()
                .map(|r| JsBatchResult {
                    key: r.key,
                    success: r.success,
                    error: r.error,
                })
                .collect())
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    #[napi]
    pub async fn get(
        &self,
        key: String,
        options: Option<JsGetOptions>,
    ) -> Result<Option<JsGetResult>> {
        let engine = Arc::clone(&self.engine);
        let opts = options.map(|o| GetOptions {
            allow_expired: o.allow_expired,
            with_ttl: o.with_ttl,
            with_expire_at: o.with_expire_at,
        });

        tokio::task::spawn_blocking(move || {
            let res = engine
                .get(&key, opts.as_ref())
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

            Ok(res.map(|r| JsGetResult {
                value: Buffer::from(r.value),
                ttl: r.ttl,
                expire_at: r.expire_at,
            }))
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    #[napi]
    pub async fn get_many(
        &self,
        keys: Vec<String>,
        options: Option<JsGetOptions>,
    ) -> Result<Vec<Option<JsGetResult>>> {
        let engine = Arc::clone(&self.engine);
        let opts = options.map(|o| GetOptions {
            allow_expired: o.allow_expired,
            with_ttl: o.with_ttl,
            with_expire_at: o.with_expire_at,
        });

        tokio::task::spawn_blocking(move || {
            let res = engine
                .get_many(&keys, opts.as_ref())
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

            Ok(res
                .into_iter()
                .map(|opt| {
                    opt.map(|r| JsGetResult {
                        value: Buffer::from(r.value),
                        ttl: r.ttl,
                        expire_at: r.expire_at,
                    })
                })
                .collect())
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    #[napi]
    pub async fn delete(&self, key: String) -> Result<bool> {
        let engine = Arc::clone(&self.engine);
        tokio::task::spawn_blocking(move || {
            engine
                .delete(&key)
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    #[napi]
    pub async fn delete_many(&self, keys: Vec<String>) -> Result<Vec<bool>> {
        let engine = Arc::clone(&self.engine);
        tokio::task::spawn_blocking(move || {
            engine
                .delete_many(&keys)
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    #[napi]
    pub async fn ttl(
        &self,
        key: String,
        options: Option<JsTtlOptions>,
    ) -> Result<Option<JsTtlResult>> {
        let engine = Arc::clone(&self.engine);
        let opts = options.map(|o| TtlOptions {
            ttl: o.ttl,
            increase: o.increase,
            reduce: o.reduce,
        });

        tokio::task::spawn_blocking(move || {
            let res = engine
                .ttl(&key, opts.as_ref())
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

            Ok(res.map(|r| JsTtlResult {
                ttl: r.ttl,
                expire_at: r.expire_at,
            }))
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    #[napi]
    pub async fn ttl_many(&self, items: Vec<JsTtlItem>) -> Result<Vec<Option<JsTtlResult>>> {
        let engine = Arc::clone(&self.engine);
        tokio::task::spawn_blocking(move || {
            let ttl_items: Vec<TtlItem> = items
                .into_iter()
                .map(|i| TtlItem {
                    key: i.key,
                    ttl: i.ttl,
                    increase: i.increase,
                    reduce: i.reduce,
                })
                .collect();

            let res = engine
                .ttl_many(&ttl_items)
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

            Ok(res
                .into_iter()
                .map(|opt| {
                    opt.map(|r| JsTtlResult {
                        ttl: r.ttl,
                        expire_at: r.expire_at,
                    })
                })
                .collect())
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    #[napi]
    pub async fn stats(&self) -> Result<JsEngineStats> {
        let engine = Arc::clone(&self.engine);
        tokio::task::spawn_blocking(move || {
            let s = engine.stats();
            Ok(JsEngineStats {
                total_keys: s.total_keys as u32,
                segments_count: s.segments_count as u32,
                live_bytes: s.live_bytes as i64,
                memory_used_bytes: s.memory_used_bytes as i64,
                ttl_cleaned_count: s.ttl_cleaned_count as i64,
                gc_cleaned_segments: s.gc_cleaned_segments as i64,
                gc_reclaimed_bytes: s.gc_reclaimed_bytes as i64,
            })
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    #[napi]
    pub async fn close(&self) -> Result<()> {
        let engine = Arc::clone(&self.engine);
        tokio::task::spawn_blocking(move || {
            engine
                .close()
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }
}

#[napi]
pub fn open_native_engine(config_json: String) -> Result<TitanKvNative> {
    let config: TitanConfig = serde_json::from_str(&config_json)
        .map_err(|e| Error::new(Status::InvalidArg, format!("Failed to parse config: {}", e)))?;

    let engine = TitanEngine::open(config)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    Ok(TitanKvNative { engine })
}
