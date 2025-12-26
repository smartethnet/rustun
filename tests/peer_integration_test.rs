/// Integration test for P2P peer handler
///
/// Tests the following scenarios:
/// 1. Peer discovery and registration
/// 2. Keepalive packet exchange
/// 3. Data frame transmission
/// 4. Connection timeout validation
/// 5. Dynamic peer addition
///
/// Note: Uses IPv6 loopback address (::1) for all peers

use rustun::codec::frame::{DataFrame, Frame, KeepAliveFrame, RouteItem};
use rustun::codec::parser::Parser;
use rustun::crypto::{Block};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;
use rustun::crypto::plain::PlainBlock;

/// Helper: Create a mock peer that listens on a UDP port
async fn create_mock_peer(
    port: u16,
    _block: Arc<Box<dyn Block>>,
) -> (UdpSocket, SocketAddr) {
    let socket = UdpSocket::bind(format!("[::1]:{}", port))
        .await
        .expect("Failed to bind mock peer socket");
    let addr = socket.local_addr().unwrap();
    tracing::info!("Mock peer listening on {}", addr);
    (socket, addr)
}

/// Helper: Receive and parse a frame from UDP socket
async fn recv_frame(socket: &UdpSocket, block: &Arc<Box<dyn Block>>) -> (Frame, SocketAddr) {
    let mut buf = vec![0u8; 2048];
    let (len, remote) = socket.recv_from(&mut buf).await.expect("Failed to recv");
    buf.truncate(len);
    let (frame, _) = Parser::unmarshal(&buf, block.as_ref()).expect("Failed to parse frame");
    (frame, remote)
}

/// Helper: Send a frame via UDP socket
async fn send_frame(
    socket: &UdpSocket,
    frame: Frame,
    remote: SocketAddr,
    block: &Arc<Box<dyn Block>>,
) {
    let data = Parser::marshal(frame, block.as_ref()).expect("Failed to marshal frame");
    socket
        .send_to(&data, remote)
        .await
        .expect("Failed to send frame");
}

/// Helper: Activate peer connection by exchanging keepalives
///
/// This ensures last_active is updated so send_frame can succeed.
///
/// # Problem
/// PeerHandler.recv_frame() filters out keepalive packets internally and only returns
/// data frames. This means calling recv_frame() after sending a keepalive would block
/// indefinitely waiting for a data frame.
///
/// # Solution
/// We send a dummy data packet after the keepalive. When recv_frame() is called:
/// 1. It processes the keepalive (updates last_active)
/// 2. It processes the dummy data and returns it
/// 3. last_active is now set and send_frame will succeed
///
/// # Returns
/// The SocketAddr of the peer (for sending future packets in tests)
async fn activate_peer_connection(
    mock_socket: &UdpSocket,
    peer_handler: &mut rustun::client::peer::PeerHandler,
    block: &Arc<Box<dyn Block>>,
) -> SocketAddr {
    // Receive initial keepalive from PeerHandler
    let (_frame, from) = recv_frame(mock_socket, block).await;
    
    // Send keepalive response (this will trigger last_active update when processed)
    send_frame(
        mock_socket,
        Frame::KeepAlive(KeepAliveFrame {
            identity: "mock_peer".to_string(),
            ipv6: "::1".to_string(),
            port: mock_socket.local_addr().unwrap().port(),
        }),
        from,
        block,
    )
    .await;
    
    // Send a dummy data packet to unblock recv_frame
    // Without this, recv_frame would wait forever since keepalives are filtered
    send_frame(
        mock_socket,
        Frame::Data(DataFrame {
            payload: b"_activation_".to_vec(),
        }),
        from,
        block,
    )
    .await;
    
    // Call recv_frame to process both packets:
    // - Keepalive: updates last_active (then filtered out)
    // - Data: returned to us (and discarded)
    let _ = tokio::time::timeout(Duration::from_millis(500), peer_handler.recv_frame())
        .await
        .ok();
    
    from
}

#[tokio::test]
async fn test_peer_keepalive_exchange() {
    // Initialize tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    tracing::info!("=== Test: Peer Keepalive Exchange ===");

    // Create crypto block
    let block: Arc<Box<dyn Block>> = Arc::new(Box::new(PlainBlock::new()));

    // Create mock peer A on port 50001
    let (mock_peer_a, _peer_a_addr) = create_mock_peer(50001, block.clone()).await;

    // Create PeerHandler on port 50000
    let mut peer_handler = rustun::client::peer::PeerHandler::new(
        block.clone(),
        "test_handler".to_string(),
        "::1".to_string(),
        50000,
    );
    peer_handler.run_peer();

    // Add peer A
    let routes = vec![RouteItem {
        identity: "peer_a".to_string(),
        private_ip: "10.0.1.2".to_string(),
        ciders: vec!["10.0.1.0/24".to_string()],
        ipv6: "::1".to_string(), // IPv6 loopback address
        port: 50001,
    }];

    peer_handler.add_peers(routes).await;

    // Wait for initial keepalive from PeerHandler
    tracing::info!("Waiting for initial keepalive from PeerHandler...");
    let (frame, from) = recv_frame(&mock_peer_a, &block).await;
    assert!(matches!(frame, Frame::KeepAlive(_)), "Expected keepalive frame");
    tracing::info!("✓ Received initial keepalive from {}", from);

    // Mock peer A sends keepalive back
    tracing::info!("Mock peer A sending keepalive back...");
    send_frame(
        &mock_peer_a,
        Frame::KeepAlive(KeepAliveFrame {
            identity: "mock_peer_a".to_string(),
            ipv6: "::1".to_string(),
            port: mock_peer_a.local_addr().unwrap().port(),
        }),
        from,
        &block,
    )
    .await;
    tracing::info!("✓ Sent keepalive response");

    // Wait a bit for PeerHandler to process
    sleep(Duration::from_millis(100)).await;

    tracing::info!("=== Test Passed: Keepalive Exchange ===");
}

#[tokio::test]
async fn test_peer_data_transmission() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    tracing::info!("=== Test: Peer Data Transmission ===");

    let block: Arc<Box<dyn Block>> = Arc::new(Box::new(PlainBlock::new()));

    // Create mock peer B on port 50011
    let (mock_peer_b, _peer_b_addr) = create_mock_peer(50011, block.clone()).await;

    // Create PeerHandler on port 50010
    let mut peer_handler = rustun::client::peer::PeerHandler::new(
        block.clone(),
        "test_handler".to_string(),
        "::1".to_string(),
        50010,
    );
    peer_handler.run_peer();

    // Add peer B
    let routes = vec![RouteItem {
        identity: "peer_b".to_string(),
        private_ip: "10.0.2.2".to_string(),
        ciders: vec!["10.0.2.0/24".to_string()],
        ipv6: "::1".to_string(), // IPv6 loopback address
        port: 50011,
    }];

    peer_handler.add_peers(routes).await;

    // Activate connection by exchanging keepalives
    tracing::info!("Activating connection with keepalive exchange...");
    let _from = activate_peer_connection(&mock_peer_b, &mut peer_handler, &block).await;
    tracing::info!("✓ Connection activated");

    // Now PeerHandler sends data to peer B's IP
    tracing::info!("Sending data frame to 10.0.2.2...");
    let test_payload = b"Hello from PeerHandler!".to_vec();
    let send_result = peer_handler
        .send_frame(
            Frame::Data(DataFrame {
                payload: test_payload.clone(),
            }),
            "10.0.2.2",
        )
        .await;

    assert!(send_result.is_ok(), "Failed to send data: {:?}", send_result);
    tracing::info!("✓ Data frame sent successfully");

    // Mock peer B receives the data
    tracing::info!("Waiting for data frame at mock peer B...");
    let (received_frame, _) = recv_frame(&mock_peer_b, &block).await;

    if let Frame::Data(data_frame) = received_frame {
        assert_eq!(
            data_frame.payload, test_payload,
            "Payload mismatch"
        );
        tracing::info!("✓ Received correct data payload: {:?}", String::from_utf8_lossy(&data_frame.payload));
    } else {
        panic!("Expected data frame, got {:?}", received_frame);
    }

    tracing::info!("=== Test Passed: Data Transmission ===");
}

#[tokio::test]
async fn test_peer_connection_timeout() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    tracing::info!("=== Test: Connection Timeout Validation ===");

    let block: Arc<Box<dyn Block>> = Arc::new(Box::new(PlainBlock::new()));

    // Create mock peer C on port 50021
    let (mock_peer_c, _) = create_mock_peer(50021, block.clone()).await;

    // Create PeerHandler on port 50020
    let mut peer_handler = rustun::client::peer::PeerHandler::new(
        block.clone(),
        "test_handler".to_string(),
        "::1".to_string(),
        50020,
    );
    peer_handler.run_peer();

    // Add peer C
    let routes = vec![RouteItem {
        identity: "peer_c".to_string(),
        private_ip: "10.0.3.2".to_string(),
        ciders: vec!["10.0.3.0/24".to_string()],
        ipv6: "::1".to_string(), // IPv6 loopback address
        port: 50021,
    }];

    peer_handler.add_peers(routes).await;

    // Wait for initial keepalive
    tracing::info!("Waiting for initial keepalive...");
    let (_frame, _from) = recv_frame(&mock_peer_c, &block).await;

    // DO NOT respond - let connection remain inactive

    // Try to send data immediately (should fail - no keepalive response)
    tracing::info!("Attempting to send data before keepalive response...");
    let send_result = peer_handler
        .send_frame(
            Frame::Data(DataFrame {
                payload: b"test".to_vec(),
            }),
            "10.0.3.2",
        )
        .await;

    assert!(
        send_result.is_err(),
        "Should fail to send without keepalive response"
    );
    tracing::info!("✓ Correctly rejected send (peer never responded): {:?}", send_result.err());

    tracing::info!("=== Test Passed: Connection Timeout ===");
}

#[tokio::test]
async fn test_peer_periodic_keepalive() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    tracing::info!("=== Test: Periodic Keepalive Timer ===");

    let block: Arc<Box<dyn Block>> = Arc::new(Box::new(PlainBlock::new()));

    // Create mock peer D on port 50031
    let (mock_peer_d, _) = create_mock_peer(50031, block.clone()).await;

    // Create PeerHandler on port 50030
    let mut peer_handler = rustun::client::peer::PeerHandler::new(
        block.clone(),
        "test_handler".to_string(),
        "::1".to_string(),
        50030,
    );
    peer_handler.run_peer();

    // Add peer D
    let routes = vec![RouteItem {
        identity: "peer_d".to_string(),
        private_ip: "10.0.4.2".to_string(),
        ciders: vec!["10.0.4.0/24".to_string()],
        ipv6: "::1".to_string(), // IPv6 loopback address
        port: 50031,
    }];

    peer_handler.add_peers(routes).await;

    // Start keepalive timer
    peer_handler.start_probe_timer().await;

    // Receive initial keepalive (from add_peers)
    tracing::info!("Receiving initial keepalive...");
    let (_frame, from) = recv_frame(&mock_peer_d, &block).await;
    tracing::info!("✓ Received initial keepalive");

    // Send response to activate connection
    send_frame(
        &mock_peer_d,
        Frame::KeepAlive(KeepAliveFrame {
            identity: "mock_peer_d".to_string(),
            ipv6: "::1".to_string(),
            port: mock_peer_d.local_addr().unwrap().port(),
        }),
        from,
        &block,
    )
    .await;

    // Note: We don't need to process this keepalive for this test,
    // as we're only testing the timer, not data transmission

    // Wait for first periodic keepalive (10 seconds)
    tracing::info!("Waiting for first periodic keepalive (10s interval)...");
    tokio::time::timeout(Duration::from_secs(12), async {
        let (frame, _) = recv_frame(&mock_peer_d, &block).await;
        assert!(matches!(frame, Frame::KeepAlive(_)));
        tracing::info!("✓ Received first periodic keepalive");
    })
    .await
    .expect("Timeout waiting for periodic keepalive");

    tracing::info!("=== Test Passed: Periodic Keepalive ===");
}

#[tokio::test]
async fn test_peer_bidirectional_data() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    tracing::info!("=== Test: Bidirectional Data Exchange ===");

    let block: Arc<Box<dyn Block>> = Arc::new(Box::new(PlainBlock::new()));

    // Create mock peer E on port 50041
    let (mock_peer_e, _peer_e_addr) = create_mock_peer(50041, block.clone()).await;

    // Create PeerHandler on port 50040
    let mut peer_handler = rustun::client::peer::PeerHandler::new(
        block.clone(),
        "test_handler".to_string(),
        "::1".to_string(),
        50040,
    );
    peer_handler.run_peer();

    // Add peer E
    let routes = vec![RouteItem {
        identity: "peer_e".to_string(),
        private_ip: "10.0.5.2".to_string(),
        ciders: vec!["10.0.5.0/24".to_string()],
        ipv6: "::1".to_string(), // IPv6 loopback address
        port: 50041,
    }];

    peer_handler.add_peers(routes).await;

    // Activate connection by exchanging keepalives
    tracing::info!("Activating connection with keepalive exchange...");
    let from = activate_peer_connection(&mock_peer_e, &mut peer_handler, &block).await;
    tracing::info!("✓ Connection activated");

    // PeerHandler -> Mock Peer (outbound)
    tracing::info!("Testing PeerHandler -> Mock Peer...");
    peer_handler
        .send_frame(
            Frame::Data(DataFrame {
                payload: b"Outbound data".to_vec(),
            }),
            "10.0.5.2",
        )
        .await
        .expect("Failed to send outbound data");

    let (received_frame, _) = recv_frame(&mock_peer_e, &block).await;
    if let Frame::Data(data) = received_frame {
        assert_eq!(data.payload, b"Outbound data");
        tracing::info!("✓ Mock peer received outbound data");
    } else {
        panic!("Expected data frame");
    }

    // Mock Peer -> PeerHandler (inbound)
    tracing::info!("Testing Mock Peer -> PeerHandler...");
    send_frame(
        &mock_peer_e,
        Frame::Data(DataFrame {
            payload: b"Inbound data".to_vec(),
        }),
        from,
        &block,
    )
    .await;

    // Spawn recv_frame task
    let received_frame = tokio::time::timeout(Duration::from_secs(2), peer_handler.recv_frame())
        .await
        .expect("Timeout receiving inbound data")
        .expect("Failed to receive inbound data");

    if let Frame::Data(data) = received_frame {
        assert_eq!(data.payload, b"Inbound data");
        tracing::info!("✓ PeerHandler received inbound data");
    } else {
        panic!("Expected data frame");
    }

    tracing::info!("=== Test Passed: Bidirectional Data ===");
}

