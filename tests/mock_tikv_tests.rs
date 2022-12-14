#[cfg(test)]
mod test {
    use grpcio::redirect_log;
    use log::debug;
    use mock_tikv::{start_mock_pd_server, start_mock_tikv_server, MOCK_PD_PORT};
    use simple_logger::SimpleLogger;
    use tikv_client::{KvPair, RawClient};

    #[tokio::test]
    #[ignore]
    async fn test_raw_put_get() {
        SimpleLogger::new().init().unwrap();
        redirect_log();

        let mut tikv_server = start_mock_tikv_server();
        let _pd_server = start_mock_pd_server();

        let client = RawClient::new(vec![format!("localhost:{MOCK_PD_PORT}")], None)
            .await
            .unwrap();

        // empty; get non-existent key
        let res = client.get("k1".to_owned()).await;
        assert_eq!(res.unwrap(), None);

        // empty; put then batch_get
        client.put("k1".to_owned(), "v1".to_owned()).await.unwrap();
        client.put("k2".to_owned(), "v2".to_owned()).await.unwrap();

        let res = client
            .batch_get(vec!["k1".to_owned(), "k2".to_owned(), "k3".to_owned()])
            .await
            .unwrap();
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].1, "v1".as_bytes());
        assert_eq!(res[1].1, "v2".as_bytes());

        // k1,k2; batch_put then batch_get
        client
            .batch_put(vec![
                KvPair::new("k3".to_owned(), "v3".to_owned()),
                KvPair::new("k4".to_owned(), "v4".to_owned()),
            ])
            .await
            .unwrap();

        let res = client
            .batch_get(vec!["k4".to_owned(), "k3".to_owned()])
            .await
            .unwrap();
        assert_eq!(res[0].1, "v3".as_bytes());
        assert_eq!(res[1].1, "v4".as_bytes());

        // k1,k2,k3,k4; delete then get
        let res = client.delete("k3".to_owned()).await;
        assert!(res.is_ok());

        let res = client.get("k3".to_owned()).await;
        assert_eq!(res.unwrap(), None);

        // k1,k2,k4; batch_delete then batch_get
        let res = client
            .batch_delete(vec!["k1".to_owned(), "k2".to_owned(), "k4".to_owned()])
            .await;
        assert!(res.is_ok());

        let res = client
            .batch_get(vec![
                "k1".to_owned(),
                "k2".to_owned(),
                "k3".to_owned(),
                "k4".to_owned(),
            ])
            .await
            .unwrap();
        assert_eq!(res.len(), 0);

        debug!("Pass all tests");

        let _ = tikv_server.shutdown().await;
        // FIXME: shutdown PD server
        // let _ = pd_server.shutdown().await;
    }
}
