use aws_sdk_dynamodb::model::AttributeValue;
use aws_sdk_dynamodb::Client;
use chrono::DateTime;
use chrono::Utc;
use http::Method;
use lambda_http::request::RequestContext;
use lambda_http::{service_fn, Error, IntoResponse, Request, RequestExt, Response};
use std::time::SystemTime;

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_http::run(service_fn(run)).await?;
    Ok(())
}

const TABLE_NAME: &str = "KvpApiDb";
const KEY: &str = "key";
const VALUE: &str = "value";
const TIME: &str = "time";

async fn put_item(client: &Client, key: &str, item: &String) -> Result<(), Error> {
    let now = SystemTime::now();
    let now: DateTime<Utc> = now.into();
    let now = now.to_rfc3339();

    client
        .put_item()
        .table_name(TABLE_NAME)
        .item(KEY, AttributeValue::S(key.to_string()))
        .item(VALUE, AttributeValue::S(item.to_string()))
        .item(TIME, AttributeValue::S(now.to_string()))
        .send()
        .await?;

    Ok(())
}

async fn get_item(client: &Client, key: &str) -> Result<Option<serde_json::value::Value>, Error> {
    let resp = client
        .get_item()
        .table_name(TABLE_NAME)
        .key(KEY, AttributeValue::S(key.to_string()))
        .send()
        .await?;

    Ok(match resp.item {
        Some(ref item) => match item[VALUE].as_s() {
            Ok(s) => Some(serde_json::from_str::<serde_json::value::Value>(s).unwrap()),
            _ => panic!(),
        },
        None => None,
    })
}

async fn delete_item(client: &Client, key: &str) -> Result<(), Error> {
    client
        .delete_item()
        .table_name(TABLE_NAME)
        .key(KEY, AttributeValue::S(key.to_string()))
        .send()
        .await?;

    Ok(())
}

async fn patch_item(
    client: &Client,
    key: &str,
    item: &serde_json::value::Value,
) -> Result<(), Error> {
    let old_item = get_item(client, key).await?;
    match old_item {
        Some(mut old_item) => Ok(json_patch::merge(&mut old_item, item)),
        None => Err("no item".into()),
    }
}

async fn run(request: Request) -> Result<impl IntoResponse, Error> {
    let shared_config = aws_config::load_from_env().await;
    let client = Client::new(&shared_config);
    let context = request.request_context();

    let resource_path = match context {
        RequestContext::ApiGatewayV1(r) => r.resource_path,
        _ => panic!(),
    };

    Ok(match resource_path {
        Some(s) => {
            let params = request.path_parameters();
            let key = params.first("key").expect("must have key");

            match s.as_str() {
                "/{key}" => match *request.method() {
                    Method::GET => match get_item(&client, key).await? {
                        Some(s) => Response::builder()
                            .status(200)
                            .body(serde_json::to_string(&s).unwrap())
                            .unwrap(),
                        None => Response::builder().status(404).body("".into()).unwrap(),
                    },
                    Method::POST => {
                        let payload = match request.body() {
                            lambda_http::Body::Text(s) => s,
                            _ => {
                                return Ok(Response::builder().status(400).body("".into()).unwrap())
                            }
                        };

                        put_item(&client, key, payload).await?;
                        Response::builder().status(204).body("".into()).unwrap()
                    }
                    Method::DELETE => {
                        delete_item(&client, key).await?;
                        Response::builder().status(204).body("".into()).unwrap()
                    }
                    Method::PATCH => {
                        let payload = match request.body() {
                            lambda_http::Body::Text(s) => s,
                            _ => {
                                return Ok(Response::builder().status(400).body("".into()).unwrap())
                            }
                        };
                        match serde_json::to_value(payload) {
                            Ok(v) => match patch_item(&client, key, &v).await {
                                Ok(_) => Response::builder().status(204).body("".into()).unwrap(),
                                Err(_) => Response::builder().status(400).body("".into()).unwrap(),
                            },
                            Err(_) => Response::builder().status(400).body("".into()).unwrap(),
                        }
                    }
                    _ => Response::builder().status(400).body("".into()).unwrap(),
                },
                _ => Response::builder().status(400).body("".into()).unwrap(),
            }
        }
        None => Response::builder().status(400).body("".into()).unwrap(),
    })
}
