use anchor_client::Cluster;
use drift_sdk::clearing_house::ClearingHouse;

#[test]
fn test_instantiate() {
    let client = ClearingHouse::default(Cluster::Devnet);
    println!("prgrom id {}", client.program_id.to_string());
}