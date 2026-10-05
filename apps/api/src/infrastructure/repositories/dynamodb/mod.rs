//! Optimistic unit of work: consistent reads, version conditions, atomic history/outbox.
//! Partition queries used for decisions must hold a versioned aggregate guard.
use crate::{
    db::DbPool,
    error::{ApiError, Result},
};
use aws_sdk_dynamodb::types::{AttributeValue as A, ConditionCheck, Put, TransactWriteItem};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;
mod admin;
mod billing;
mod identity;
mod jobs;
mod mail;
mod organizations;
#[cfg(test)]
mod tests;

type Key = (String, String);
#[derive(Clone)]
struct Record {
    version: Option<i64>,
    data: Value,
}
pub(crate) struct UnitOfWork<'a> {
    pool: &'a DbPool,
    runtime: tokio::runtime::Handle,
    reads: BTreeMap<Key, Record>,
    writes: BTreeMap<Key, Value>,
    audits: Vec<(Key, Value)>,
    actor: (String, Option<Uuid>),
    in_transaction: bool,
}
fn conflict() -> ApiError {
    ApiError(
        409,
        "write_conflict",
        "Data changed concurrently. Please retry.",
    )
}
fn missing() -> ApiError {
    ApiError(404, "not_found", "This item is unavailable.")
}
fn decode<T: DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(|_| ApiError::internal("Invalid persisted record"))
}
fn id(v: &Value) -> Result<Uuid> {
    decode(v["id"].clone())
}
fn active(v: &Value) -> bool {
    !v.is_null() && v["deleted_at"].is_null()
}
fn enabled(v: &Value) -> bool {
    active(v) && v["status"] == "active"
}
fn now() -> chrono::DateTime<chrono::Utc> {
    crate::infrastructure::clock::now()
}
fn expires(v: &Value, field: &str) -> bool {
    v[field]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .is_some_and(|t| t > now())
}
fn key(model: &str, id: impl std::fmt::Display) -> Key {
    (format!("{model}#{id}"), "record".into())
}
fn keys(k: &Key) -> HashMap<String, A> {
    HashMap::from([
        ("pk".into(), A::S(k.0.clone())),
        ("sk".into(), A::S(k.1.clone())),
    ])
}
fn number(v: &Value, field: &str) -> i64 {
    v[field].as_i64().unwrap_or(0)
}
fn fresh(mut data: Value) -> Value {
    data["id"] = json!(Uuid::new_v4());
    data["created_at"] = json!(now());
    data["deleted_at"] = Value::Null;
    data
}
impl UnitOfWork<'_> {
    fn get(&mut self, k: &Key) -> Result<Value> {
        if let Some(v) = self.writes.get(k) {
            return Ok(v.clone());
        }
        if let Some(v) = self.reads.get(k) {
            return Ok(v.data.clone());
        }
        let item = self
            .runtime
            .block_on(
                self.pool
                    .client
                    .get_item()
                    .table_name(&self.pool.table)
                    .set_key(Some(keys(k)))
                    .consistent_read(true)
                    .send(),
            )
            .map_err(|_| ApiError::internal("DynamoDB read failed"))?
            .item;
        let record = match item {
            Some(item) => Record {
                version: Some(
                    item.get("version")
                        .and_then(|a| a.as_n().ok())
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| ApiError::internal("Missing record version"))?,
                ),
                data: serde_json::from_str(
                    item.get("data")
                        .and_then(|a| a.as_s().ok())
                        .ok_or_else(|| ApiError::internal("Missing record data"))?,
                )
                .map_err(|_| ApiError::internal("Invalid record data"))?,
            },
            None => Record {
                version: None,
                data: Value::Null,
            },
        };
        let data = record.data.clone();
        self.reads.insert(k.clone(), record);
        Ok(data)
    }
    /// Presentation-only reads do not expand a login transaction with every organization.
    /// Authorization and entitlement decisions must use `get`/`require` instead.
    fn projection(&mut self, k: &Key) -> Result<Value> {
        let tracked = self.reads.contains_key(k);
        let value = self.get(k)?;
        if !tracked && !self.writes.contains_key(k) {
            self.reads.remove(k);
        }
        Ok(value)
    }
    fn require(&mut self, k: &Key) -> Result<Value> {
        let v = self.get(k)?;
        if active(&v) { Ok(v) } else { Err(missing()) }
    }
    fn save(&mut self, k: Key, mut v: Value) -> Result<()> {
        let old = self.get(&k)?;
        if !old.is_null() && old["id"] != v["id"] {
            return Err(ApiError::internal("Record IDs are immutable"));
        }
        if !v["deleted_at"].is_null() && old["deleted_at"].is_null() {
            v["deleted_by"] = json!(self.actor.1);
            v["deleted_by_kind"] = json!(self.actor.0);
        } else if v["deleted_at"].is_null() {
            v["deleted_by"] = Value::Null;
            v["deleted_by_kind"] = Value::Null;
        }
        let mut changes = serde_json::Map::new();
        if let Some(fields) = v.as_object() {
            for (field, value) in fields {
                if old.get(field) != Some(value) {
                    let secret = [
                        "password_hash",
                        "token_hash",
                        "body",
                        "html_body",
                        "payload",
                        "parameters",
                    ]
                    .contains(&field.as_str());
                    changes.insert(field.clone(), json!({"from": if secret {json!("[redacted]")} else {old[field].clone()}, "to": if secret {json!("[redacted]")} else {value.clone()}}));
                }
            }
        }
        let audit_id = Uuid::new_v4();
        let audit = json!({"id":audit_id,"record_id":v["id"],"operation":if old.is_null(){"INSERT"}else{"UPDATE"},"changes":changes,"actor_id":self.actor.1,"actor_kind":self.actor.0,"changed_at":now()});
        self.audits.push((
            (
                format!("history#{}", id(&v)?),
                format!("{}#{audit_id}", now().to_rfc3339()),
            ),
            audit,
        ));
        self.writes.insert(k, v);
        if !self.in_transaction {
            self.commit()?;
            self.clear();
        }
        Ok(())
    }
    fn remove(&mut self, k: Key) -> Result<usize> {
        let mut v = self.get(&k)?;
        if !active(&v) {
            return Ok(0);
        }
        v["deleted_at"] = json!(now());
        self.save(k, v)?;
        Ok(1)
    }
    fn reserve(&mut self, k: Key, owner: Uuid) -> Result<()> {
        let mut v = self.get(&k)?;
        if active(&v) {
            if v["owner"] == json!(owner) {
                return Ok(());
            }
            return Err(ApiError(409, "conflict", "This value is already in use."));
        }
        if v.is_null() {
            v = fresh(json!({}));
        }
        v["owner"] = json!(owner);
        v["deleted_at"] = Value::Null;
        self.save(k, v)
    }
    fn change_email_reservation(
        &mut self,
        kind: &str,
        old: &Value,
        email: &str,
        owner: Uuid,
    ) -> Result<()> {
        self.reserve(key(kind, email), owner)?;
        if let Some(previous) = old.as_str().filter(|previous| *previous != email) {
            self.remove(key(kind, previous))?;
        }
        Ok(())
    }
    fn touch(&mut self, k: Key) -> Result<()> {
        let mut v = self.get(&k)?;
        if v.is_null() {
            v = fresh(json!({}));
        }
        v["revision"] = json!(number(&v, "revision") + 1);
        self.save(k, v)
    }
    fn clear(&mut self) {
        self.reads.clear();
        self.writes.clear();
        self.audits.clear();
        self.actor = ("system".into(), None);
    }
    pub(crate) fn actor(&mut self, kind: &str, id: Uuid) -> Result<()> {
        if !self.in_transaction {
            return Err(ApiError::internal(
                "Audit attribution requires a transaction",
            ));
        }
        self.actor = (kind.into(), Some(id));
        Ok(())
    }
    pub(crate) fn generated_id(&mut self) -> Result<Uuid> {
        Ok(Uuid::new_v4())
    }
    pub(crate) fn transaction<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if self.in_transaction {
            return Err(ApiError::internal("Nested transactions are unsupported"));
        }
        self.clear();
        self.in_transaction = true;
        let result = f(self).and_then(|value| {
            self.commit()?;
            Ok(value)
        });
        self.in_transaction = false;
        self.clear();
        result
    }
    fn commit(&mut self) -> Result<()> {
        if self.writes.is_empty() {
            return Ok(());
        }
        let mut operations = Vec::new();
        for (k, r) in &self.reads {
            let condition = if r.version.is_some() {
                "#v = :v"
            } else {
                "attribute_not_exists(pk)"
            };
            let values = r
                .version
                .map(|v| HashMap::from([(":v".into(), A::N(v.to_string()))]));
            let names = r
                .version
                .map(|_| HashMap::from([("#v".into(), "version".into())]));
            if let Some(v) = self.writes.get(k) {
                let mut item = keys(k);
                item.insert(
                    "version".into(),
                    A::N((r.version.unwrap_or(0) + 1).to_string()),
                );
                item.insert("data".into(), A::S(v.to_string()));
                if let Some(gpk) = v["_listing"].as_str() {
                    item.insert("gpk".into(), A::S(gpk.into()));
                    item.insert(
                        "gsk".into(),
                        A::S(format!(
                            "{}#{}",
                            v["created_at"].as_str().unwrap_or_default(),
                            v["id"].as_str().unwrap_or_default()
                        )),
                    );
                }
                operations.push(
                    TransactWriteItem::builder()
                        .put(
                            Put::builder()
                                .table_name(&self.pool.table)
                                .set_item(Some(item))
                                .condition_expression(condition)
                                .set_expression_attribute_names(names)
                                .set_expression_attribute_values(values)
                                .build()
                                .map_err(ApiError::internal)?,
                        )
                        .build(),
                );
            } else {
                operations.push(
                    TransactWriteItem::builder()
                        .condition_check(
                            ConditionCheck::builder()
                                .table_name(&self.pool.table)
                                .set_key(Some(keys(k)))
                                .condition_expression(condition)
                                .set_expression_attribute_names(names)
                                .set_expression_attribute_values(values)
                                .build()
                                .map_err(ApiError::internal)?,
                        )
                        .build(),
                );
            }
        }
        for (k, v) in &self.audits {
            let mut item = keys(k);
            item.insert("version".into(), A::N("1".into()));
            item.insert("data".into(), A::S(v.to_string()));
            operations.push(
                TransactWriteItem::builder()
                    .put(
                        Put::builder()
                            .table_name(&self.pool.table)
                            .set_item(Some(item))
                            .condition_expression("attribute_not_exists(pk)")
                            .build()
                            .map_err(ApiError::internal)?,
                    )
                    .build(),
            );
        }
        if operations.len() > 100 {
            return Err(ApiError(
                409,
                "transaction_limit",
                "This operation exceeds the storage transaction limit.",
            ));
        }
        use aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError;
        let token = Uuid::new_v4().to_string();
        for attempt in 0..4 {
            let result = self.runtime.block_on(
                self.pool
                    .client
                    .transact_write_items()
                    .set_transact_items(Some(operations.clone()))
                    .client_request_token(&token)
                    .send(),
            );
            match result {
                Ok(_) => return Ok(()),
                Err(error) => {
                    if let Some(TransactWriteItemsError::TransactionCanceledException(cancelled)) =
                        error.as_service_error()
                    {
                        let codes: Vec<_> = cancelled
                            .cancellation_reasons()
                            .iter()
                            .filter_map(|reason| reason.code())
                            .filter(|code| *code != "None")
                            .collect();
                        if !codes.is_empty()
                            && codes.iter().all(|code| *code == "TransactionConflict")
                            && attempt < 3
                        {
                            // Retry the identical conditional request, never replay business effects.
                            std::thread::sleep(std::time::Duration::from_millis(
                                25 * (1 << attempt),
                            ));
                            continue;
                        }
                        tracing::warn!(?codes, "DynamoDB transaction cancelled");
                        return Err(
                            if codes.iter().any(|code| {
                                ["ConditionalCheckFailed", "TransactionConflict"].contains(code)
                            }) {
                                conflict()
                            } else {
                                ApiError::internal("DynamoDB transaction rejected")
                            },
                        );
                    }
                    return Err(ApiError::internal("DynamoDB commit failed"));
                }
            }
        }
        Err(conflict())
    }

    fn query(&mut self, pk: &str, prefix: &str, index: bool) -> Result<Vec<(Key, Value)>> {
        let mut start = None;
        let mut result = BTreeMap::new();
        loop {
            let query = self
                .pool
                .client
                .query()
                .table_name(&self.pool.table)
                .set_index_name(index.then(|| "listing".into()))
                .consistent_read(!index)
                .key_condition_expression(if index {
                    "gpk = :pk"
                } else if prefix.is_empty() {
                    "pk = :pk"
                } else {
                    "pk = :pk AND begins_with(sk, :prefix)"
                })
                .expression_attribute_values(":pk", A::S(pk.into()));
            let query = if index || prefix.is_empty() {
                query
            } else {
                query.expression_attribute_values(":prefix", A::S(prefix.into()))
            };
            let page = self
                .runtime
                .block_on(query.set_exclusive_start_key(start).send())
                .map_err(|_| ApiError::internal("DynamoDB query failed"))?;
            for item in page.items() {
                let string = |name| {
                    item.get(name)
                        .and_then(|a| a.as_s().ok())
                        .cloned()
                        .ok_or_else(|| ApiError::internal("Invalid query record"))
                };
                result.insert(
                    (string("pk")?, string("sk")?),
                    serde_json::from_str(&string("data")?)
                        .map_err(|_| ApiError::internal("Invalid query data"))?,
                );
            }
            start = page.last_evaluated_key;
            if start.as_ref().is_none_or(HashMap::is_empty) {
                break;
            }
        }
        for (k, v) in &self.writes {
            if (!index && k.0 == pk && k.1.starts_with(prefix)) || (index && v["_listing"] == pk) {
                result.insert(k.clone(), v.clone());
            }
        }
        Ok(result.into_iter().collect())
    }
    pub(crate) fn health_check(&mut self) -> Result<usize> {
        self.runtime
            .block_on(
                self.pool
                    .client
                    .describe_table()
                    .table_name(&self.pool.table)
                    .send(),
            )
            .map_err(|_| ApiError::internal("DynamoDB is unavailable"))?;
        Ok(1)
    }
}
pub(crate) async fn run<T: Send + 'static>(
    pool: DbPool,
    f: impl FnOnce(&mut UnitOfWork<'_>) -> Result<T> + Send + 'static,
) -> Result<T> {
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        f(&mut UnitOfWork {
            pool: &pool,
            runtime,
            reads: BTreeMap::new(),
            writes: BTreeMap::new(),
            audits: Vec::new(),
            actor: ("system".into(), None),
            in_transaction: false,
        })
    })
    .await
    .map_err(ApiError::internal)?
}
