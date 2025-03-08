use libp2p::{gossipsub, mdns};
use p2p::Event;
use patron::Patron;
use rand::Rng; // 0.8
use rand::distr::Alphanumeric;
use env_logger;
use log::{debug, error, info, log_enabled, warn, Level};

#[tokio::main]
async fn main() {

    env_logger::init();

    let mut patron = match Patron::serve().await {
        Ok(patron) => patron,
        Err(e) => return error!("Error: {:?}", e),
    };

    loop {
        match patron.rx.recv().await {
            Some(event) => match event {
                Event::Mdns(mdns::Event::Discovered(list)) => {
                    for (peer_id, addr) in list {
                        info!("Discovered: {:?} at {:?}", peer_id, addr);

                        match patron.send_message(
                            rand::rng()
                                .sample_iter(&Alphanumeric)
                                .take(7)
                                .map(char::from)
                                .collect(),
                        ) {
                            Ok(_) => info!("message sent"),
                            Err(e) => error!("Error: {:?}", e),
                        };
                    }
                }
                Event::Mdns(mdns::Event::Expired(list)) => {
                    for (peer_id, addr) in list {
                        warn!("Expired: {:?} at {:?}", peer_id, addr);
                    }
                }
                Event::Gossipsub(gossipsub::Event::Message {
                    propagation_source,
                    message_id,
                    message,
                }) => {
                    info!(
                        "Message from {:?} with id {:?}: {:?}",
                        propagation_source, message_id, message
                    );
                }
                _ => {}
            },
            None => {},
        }
    }
}
