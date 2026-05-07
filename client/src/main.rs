use anyhow::Result;
use client::GameClient;
use protocol::ActionType;

#[tokio::main]
async fn main() -> Result<()> {
    let server_addr: std::net::SocketAddr = "127.0.0.1:9876".parse()?;
    let mut client = GameClient::connect(server_addr, "test_client").await?;

    println!(
        "Initial state: pos=({},{}), hp={}/{}, entities={}",
        client.state.player_pos.x,
        client.state.player_pos.y,
        client.state.hp,
        client.state.max_hp,
        client.state.entities.len()
    );

    // Test: send a move action with prediction
    let predicted = client.send_move(ActionType::MoveRight).await?;
    println!(
        "Sent MoveRight: predicted pos=({},{})",
        predicted.x, predicted.y
    );
    assert_eq!(client.state.player_pos, predicted, "local prediction applied");

    // Tick to receive server confirmation (WorldState from AI tick)
    if client.tick().await? {
        println!(
            "After tick: confirmed pos=({},{}), seq={}",
            client.state.player_pos.x,
            client.state.player_pos.y,
            client.state.seq
        );
    }

    // Test: send wait
    client.send_wait().await?;
    println!("Sent Wait");

    // Tick to receive the WorldState broadcast
    if client.tick().await? {
        println!(
            "Final state: pos=({},{}), seq={}, pending={}",
            client.state.player_pos.x,
            client.state.player_pos.y,
            client.state.seq,
            client.input_queue.pending_count()
        );
        assert_eq!(client.input_queue.pending_count(), 0, "all actions acknowledged");
    }

    client.disconnect().await?;
    println!("Disconnected. Test passed.");
    Ok(())
}
