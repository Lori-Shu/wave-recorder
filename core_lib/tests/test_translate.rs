use md5::{Digest, Md5};
use reqwest::{
    Client,
    header::{HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};

const TRANSLATE_KEY: &str = include_str!("../resources/translate_key");
const APP_ID: &str = "20260311002570648";
const BAIDU_TRANSLATE_URL: &str = "https://fanyi-api.baidu.com/api/trans/vip/translate";
const SALT: &str = "江畔何人初见月";
pub struct Translater {
    client: Client,
    str_receiver: Receiver<String>,
    str_sender: Sender<String>,
}
impl Translater {
    pub fn new(str_receiver: Receiver<String>, str_sender: Sender<String>) -> anyhow::Result<Self> {
        let mut header = HeaderMap::new();
        let res = header.append(
            "content-type",
            HeaderValue::from_str("application/x-www-form-urlencoded")?,
        );
        println!("append header {}", res);
        let client = Client::builder().default_headers(header).build()?;
        Ok(Self {
            client,
            str_receiver,
            str_sender,
        })
    }
    pub async fn send_translate_request(&self) -> anyhow::Result<()> {
        let mut sign_str = APP_ID.to_string();
        sign_str.push_str("抽刀断水水更流");
        sign_str.push_str(SALT);
        sign_str.push_str(TRANSLATE_KEY);
        let mut md5_wrapper = <Md5 as Digest>::new();
        md5_wrapper.update(sign_str.as_bytes());
        let res = md5_wrapper.finalize();
        let sign = format!("{:x}", res);

        let params = [
            ("q", "抽刀断水水更流"),
            ("from", "zh"),
            ("to", "en"),
            ("appid", APP_ID),
            ("salt", SALT),
            ("sign", &sign),
        ];
        let response = self
            .client
            .post(BAIDU_TRANSLATE_URL)
            .form(&params)
            .send()
            .await?;
        println!("request response got");
        if response.status().is_success() {
            let translate_res =
                serde_json::from_slice::<TranslateResponse>(&(response.bytes().await?))?;
            println!(
                "translate response{}",
                serde_json::to_string(&translate_res)?
            );
        } else {
            println!("{}", response.status());
        }
        Ok(())
    }
}
#[derive(Debug, Serialize, Deserialize)]
struct TranslateResult {
    src: String,
    dst: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct TranslateResponse {
    from: String,
    to: String,
    trans_result: Vec<TranslateResult>,
}
#[cfg(test)]
mod test {
    use std::io::stdin;

    use tokio::sync::mpsc;

    use crate::Translater;

    #[test]
    fn test_translate() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let channel_0 = mpsc::channel(1);
        let channel_1 = mpsc::channel(1);
        let translater = Translater::new(channel_0.1, channel_1.0).unwrap();
        rt.block_on(translater.send_translate_request()).unwrap();
        stdin().read_line(&mut String::new()).unwrap();
    }
}
