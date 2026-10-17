//! Both catalogue requests must start before either response is required.
use beanfun_lib::services::beanfun::{
    games::list_games, BeanfunClient, ClientConfig, Endpoints, LoginRegion,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Barrier;
use url::Url;

#[tokio::test]
async fn catalogue_requests_overlap_for_both_regions() {
    for region in [LoginRegion::TW, LoginRegion::HK] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let server = tokio::spawn(async move {
            let mut handlers = Vec::new();
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let barrier = barrier.clone();
                handlers.push(tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buffer = [0; 1024];
                    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        let count = stream.read(&mut buffer).await.unwrap();
                        assert!(count > 0);
                        request.extend_from_slice(&buffer[..count]);
                    }
                    let request = String::from_utf8(request).unwrap();
                    let body = if request.starts_with("GET /beanfun_block/generic_handlers/get_service_ini.ashx ") {
                        "[610074_T9]\nexe=Maple.exe\n"
                    } else {
                        assert!(request.starts_with("GET /game_zone/ "));
                        "<script>Services.ServiceList = [{\"ServiceCode\":\"610074\",\"ServiceRegion\":\"T9\"}];</script>"
                    };
                    barrier.wait().await;
                    let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    stream.write_all(response.as_bytes()).await.unwrap();
                }));
            }
            for handler in handlers {
                handler.await.unwrap();
            }
        });
        let mut config = ClientConfig::for_region(region);
        config.endpoints = Endpoints {
            login_base: base.clone(),
            portal_base: base.clone(),
            newlogin_base: base,
        };
        let client = BeanfunClient::new(config).unwrap();
        let bundle = tokio::time::timeout(Duration::from_secs(5), list_games(&client))
            .await
            .expect("both requests must arrive before responses are released")
            .unwrap();
        assert_eq!(bundle.ini["610074_T9"].exe, "Maple.exe");
        assert_eq!(bundle.services[0].service_code, "610074");
        server.await.unwrap();
    }
}
