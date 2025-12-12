use std::collections::HashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};


// pub struct MagicValue<T>(T);

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    host: String,
    path: String,
    method: String,
    headers: Vec<Header>,
    body: String,
    cookies: Vec<(String, String)>,
    query: Vec<(String, String)>,
    params: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Header {
    key: String,
    typ: String,
    value: String,
}

#[allow(unused)]
async fn xx() -> Result<(), Box<dyn std::error::Error>> {
    let builder = Client::builder();
    // builder.build()?.post("").multipart();

    let resp = reqwest::get("https://httpbin.org/ip");

    drop(resp);

    // println!("{resp:#?}");

    Ok(())
}