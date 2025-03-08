use libp2p::{gossipsub, mdns};
use p2p::Event;
use patron::Patron;
use rand::Rng; // 0.8
use rand::distr::Alphanumeric;

#[tokio::main]
async fn main() {
    let mut patron = match Patron::serve().await {
        Ok(patron) => patron,
        Err(e) => return eprintln!("Error: {:?}", e),
    };

    loop {
        match patron.rx.recv().await {
            Some(event) => match event {
                Event::Mdns(mdns::Event::Discovered(list)) => {
                    for (peer_id, addr) in list {
                        println!("Discovered: {:?} at {:?}", peer_id, addr);

                        match patron.send_message(
                            rand::rng()
                                .sample_iter(&Alphanumeric)
                                .take(7)
                                .map(char::from)
                                .collect(),
                        ) {
                            Ok(_) => println!("message sent"),
                            Err(e) => eprintln!("Error: {:?}", e),
                        };
                    }
                }
                Event::Mdns(mdns::Event::Expired(list)) => {
                    for (peer_id, addr) in list {
                        println!("Expired: {:?} at {:?}", peer_id, addr);
                    }
                }
                Event::Gossipsub(gossipsub::Event::Message {
                    propagation_source,
                    message_id,
                    message,
                }) => {
                    println!(
                        "Message from {:?} with id {:?}: {:?}",
                        propagation_source, message_id, message
                    );
                }
                _ => {}
            },
            None => break,
        }
    }
}
