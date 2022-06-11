use aws_sdk_dynamodb::model::AttributeValue;
use aws_sdk_dynamodb::Client;
use chrono::DateTime;
use chrono::Utc;
use lambda_http::Error;
use std::time::SystemTime;

const TABLE_NAME: &str = "KvpApiDb";
const KEY: &str = "key";
const VALUE: &str = "value";
const TIME: &str = "time";

pub async fn put_item(client: &Client, key: &str, item: &serde_json::Value) -> Result<(), Error> {
    let now = SystemTime::now();
    let now: DateTime<Utc> = now.into();
    let now = now.to_rfc3339();

    client
        .put_item()
        .table_name(TABLE_NAME)
        .item(KEY, AttributeValue::S(key.to_string()))
        .item(
            VALUE,
            AttributeValue::S(serde_json::to_string(item).unwrap()),
        )
        .item(TIME, AttributeValue::S(now.to_string()))
        .send()
        .await?;

    Ok(())
}

pub async fn get_item(client: &Client, key: &str) -> Result<Option<serde_json::Value>, Error> {
    let resp = client
        .get_item()
        .table_name(TABLE_NAME)
        .key(KEY, AttributeValue::S(key.to_string()))
        .send()
        .await?;

    Ok(match resp.item {
        Some(ref item) => match item[VALUE].as_s() {
            Ok(s) => Some(serde_json::from_str::<serde_json::Value>(s).unwrap()),
            _ => panic!(),
        },
        None => None,
    })
}

pub async fn delete_item(client: &Client, key: &str) -> Result<(), Error> {
    client
        .delete_item()
        .table_name(TABLE_NAME)
        .key(KEY, AttributeValue::S(key.to_string()))
        .send()
        .await?;

    Ok(())
}

pub async fn patch_item(client: &Client, key: &str, item: &serde_json::Value) -> Result<(), Error> {
    let old_item = get_item(client, key).await?;
    match old_item {
        Some(mut old_item) => {
            json_patch::merge(&mut old_item, item);
            put_item(&client, key, &old_item).await?;
            Ok(())
        }
        None => Ok(put_item(client, key, item).await?),
    }
}
