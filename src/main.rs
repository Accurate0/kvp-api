use aws_sdk_dynamodb::Client;
use http::Method;
use lambda_http::request::RequestContext;
use lambda_http::{service_fn, Error, Request, RequestExt, Response};

mod db;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let shared_config = aws_config::load_from_env().await;

    let client = Client::new(&shared_config);
    let client = &client;

    let handler_func_closure = move |request: Request| async move {
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
                        Method::GET => match db::get_item(&client, key).await? {
                            Some(s) => Response::builder()
                                .status(200)
                                .body(serde_json::to_string(&s)?)?,
                            None => Response::builder().status(404).body("".into())?,
                        },
                        Method::POST => match request.body() {
                            lambda_http::Body::Text(s) => {
                                match serde_json::from_str::<serde_json::Value>(s) {
                                    Ok(obj) => {
                                        db::put_item(&client, key, &obj).await?;
                                        Response::builder().status(204).body("".into())?
                                    }
                                    Err(_) => Response::builder().status(400).body("".into())?,
                                }
                            }
                            _ => return Ok(Response::builder().status(400).body("".into())?),
                        },
                        Method::DELETE => {
                            db::delete_item(&client, key).await?;
                            Response::builder().status(204).body("".into())?
                        }
                        Method::PATCH => {
                            let payload = match request.body() {
                                lambda_http::Body::Text(s) => s,
                                _ => return Ok(Response::builder().status(400).body("".into())?),
                            };
                            match serde_json::from_str::<serde_json::Value>(payload) {
                                Ok(v) => match db::patch_item(&client, key, &v).await {
                                    Ok(_) => Response::builder().status(204).body("".into())?,
                                    Err(_) => Response::builder().status(400).body("".into())?,
                                },
                                Err(_) => Response::builder().status(415).body("".into())?,
                            }
                        }
                        _ => Response::builder().status(400).body("".into())?,
                    },
                    _ => Response::builder().status(400).body("".into())?,
                }
            }
            None => Response::builder().status(400).body("".into())?,
        })
    };

    lambda_http::run(service_fn(handler_func_closure)).await?;
    Ok(())
}
