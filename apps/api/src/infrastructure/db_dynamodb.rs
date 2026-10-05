//! DynamoDB composition. Tables are provisioned by SAM; local bootstrap is explicit.
use crate::error::{ApiError, Result};
use aws_sdk_dynamodb::{
    Client,
    types::{
        AttributeDefinition, BillingMode, GlobalSecondaryIndex, KeySchemaElement, KeyType,
        Projection, ProjectionType, ScalarAttributeType,
    },
};

#[derive(Clone)]
pub struct DbPool {
    pub(crate) client: Client,
    pub(crate) table: String,
}
pub async fn from_env() -> Result<DbPool> {
    let table = std::env::var("DYNAMODB_TABLE")
        .map_err(|_| ApiError::internal("DYNAMODB_TABLE is required"))?;
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .timeout_config(
            aws_config::timeout::TimeoutConfig::builder()
                .operation_timeout(std::time::Duration::from_secs(10))
                .build(),
        )
        .load()
        .await;
    let mut builder = aws_sdk_dynamodb::config::Builder::from(&config);
    if let Ok(endpoint) = std::env::var("DYNAMODB_ENDPOINT_URL") {
        if std::env::var("APP_ENV").as_deref() == Ok("production") {
            return Err(ApiError::internal(
                "DynamoDB endpoint overrides are development-only",
            ));
        }
        builder = builder.endpoint_url(endpoint);
    }
    Ok(DbPool {
        client: Client::from_conf(builder.build()),
        table,
    })
}
pub async fn migrate(pool: DbPool) -> Result<()> {
    let result = pool
        .client
        .describe_table()
        .table_name(&pool.table)
        .send()
        .await;
    if result.as_ref().err().is_some_and(|e| {
        e.as_service_error()
            .is_some_and(|e| e.is_resource_not_found_exception())
    }) && std::env::var("DYNAMODB_CREATE_TABLE").as_deref() == Ok("true")
        && std::env::var("APP_ENV").as_deref() != Ok("production")
        && std::env::var("DYNAMODB_ENDPOINT_URL").is_ok()
    {
        let key = |name, kind| {
            KeySchemaElement::builder()
                .attribute_name(name)
                .key_type(kind)
                .build()
                .map_err(ApiError::internal)
        };
        let attr = |name| {
            AttributeDefinition::builder()
                .attribute_name(name)
                .attribute_type(ScalarAttributeType::S)
                .build()
                .map_err(ApiError::internal)
        };
        pool.client
            .create_table()
            .table_name(&pool.table)
            .billing_mode(BillingMode::PayPerRequest)
            .attribute_definitions(attr("pk")?)
            .attribute_definitions(attr("sk")?)
            .attribute_definitions(attr("gpk")?)
            .attribute_definitions(attr("gsk")?)
            .key_schema(key("pk", KeyType::Hash)?)
            .key_schema(key("sk", KeyType::Range)?)
            .global_secondary_indexes(
                GlobalSecondaryIndex::builder()
                    .index_name("listing")
                    .key_schema(key("gpk", KeyType::Hash)?)
                    .key_schema(key("gsk", KeyType::Range)?)
                    .projection(
                        Projection::builder()
                            .projection_type(ProjectionType::All)
                            .build(),
                    )
                    .build()
                    .map_err(ApiError::internal)?,
            )
            .send()
            .await
            .map_err(|_| ApiError::internal("DynamoDB table creation failed"))?;
    } else {
        result.map_err(|_| ApiError::internal("DynamoDB table is unavailable"))?;
    }
    let table = pool
        .client
        .describe_table()
        .table_name(&pool.table)
        .send()
        .await
        .map_err(|_| ApiError::internal("DynamoDB table is unavailable"))?
        .table
        .ok_or_else(|| ApiError::internal("Missing table description"))?;
    if !table
        .key_schema()
        .iter()
        .any(|k| k.attribute_name() == "pk" && k.key_type() == &KeyType::Hash)
        || !table
            .key_schema()
            .iter()
            .any(|k| k.attribute_name() == "sk" && k.key_type() == &KeyType::Range)
        || !table.global_secondary_indexes().iter().any(|i| {
            i.index_name() == Some("listing")
                && i.key_schema()
                    .iter()
                    .any(|k| k.attribute_name() == "gpk" && k.key_type() == &KeyType::Hash)
                && i.key_schema()
                    .iter()
                    .any(|k| k.attribute_name() == "gsk" && k.key_type() == &KeyType::Range)
                && i.projection().and_then(|p| p.projection_type()) == Some(&ProjectionType::All)
        })
        || ["pk", "sk", "gpk", "gsk"].iter().any(|name| {
            !table.attribute_definitions().iter().any(|a| {
                a.attribute_name() == *name && a.attribute_type() == &ScalarAttributeType::S
            })
        })
    {
        return Err(ApiError::internal(
            "DynamoDB table schema does not match AppShell",
        ));
    }
    Ok(())
}
