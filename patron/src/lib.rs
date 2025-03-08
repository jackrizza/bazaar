use libp2p::Swarm;
use tokio::sync::mpsc;

use env_logger;
use log::{debug, error, info, log_enabled, warn, Level};

use p2p::{CoreBehaviour, Event, P2P};

// We create a custom network behaviour that combines Gossipsub and Mdns.
pub struct Patron {
    pub rx: mpsc::Receiver<Event>,
    tx: mpsc::Sender<String>,
}

impl Patron {
    pub async fn serve() -> Result<Self, Box<dyn std::error::Error>> {
        let swarm: Swarm<CoreBehaviour> = match P2P::swarm_build() {
            Ok(s) => s,
            Err(e) => return Err(e),
        };
        let p2p_wrapper = P2P::new(swarm);

        match p2p_wrapper.serve("Patron").await {
            Ok((tx, rx)) => Ok(Patron { rx, tx }),
            Err(e) => Err(e),
        }
    }

    pub fn send_message(&self, message: String) -> Result<(), Box<dyn std::error::Error>> {
        match self.tx.try_send(message) {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
