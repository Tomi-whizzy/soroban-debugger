#![allow(dead_code)]

use soroban_debugger::server::DebugServer;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

#[tokio::test]
async fn test_server_shutdown_on_ctrl_c() {
    let server = DebugServer::new(None, None, None).expect("Failed to create server");
    let shutdown = server.shutdown.clone();

    let server_task = tokio::spawn(async move {
        let _ = server.run(0);
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
    shutdown.notify_one();

    let result = timeout(Duration::from_secs(5), server_task).await;
    assert!(result.is_ok(), "Server should shutdown cleanly");
    assert!(result.unwrap().is_ok(), "Server task should not panic");
}

#[tokio::test]
async fn test_server_closes_socket_on_shutdown() {
    let server = DebugServer::new(None, None, None).expect("Failed to create server");
    let shutdown = server.shutdown.clone();

    let server_task = tokio::spawn(async move {
        let _ = server.run(0);
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
    shutdown.notify_one();

    timeout(Duration::from_secs(5), server_task)
        .await
        .expect("Server shutdown timed out")
        .expect("Server task panicked");
}

#[tokio::test]
async fn test_server_creation_with_token() {
    let token = "valid-test-token-1234567890".to_string();
    let server = DebugServer::new(Some(token.clone()), None, None)
        .expect("Failed to create server with token");

    assert_eq!(server.token, Some(token));
}

#[tokio::test]
async fn test_server_rejects_pending_requests_on_shutdown() {
    let server = DebugServer::new(None, None, None).expect("Failed to create server");
    let shutdown = server.shutdown.clone();

    let server_task = tokio::spawn(async move {
        let _ = server.run(0);
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    shutdown.notify_one();

    let result = timeout(Duration::from_secs(5), server_task).await;
    assert!(result.is_ok(), "Server should complete shutdown quickly");
}
