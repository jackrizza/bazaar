use std::{
    collections::hash_map::DefaultHasher,
    error::Error,
    hash::{Hash, Hasher},
    time::Duration,
};

use tokio::{select, sync::mpsc};

use futures::stream::StreamExt;
use libp2p::{
    gossipsub, mdns, noise,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux,
};
use tokio::io::{self};

// Define a unified event type for our behaviour.
#[derive(Debug)]
pub enum Event {
    Gossipsub(gossipsub::Event),
    Mdns(mdns::Event),
}

// Use the libp2p derive macro to combine our behaviours.
// The `#[behaviour(out_event = "Event")]` attribute tells libp2p to wrap
// the events coming from each sub-behaviour into our unified Event enum.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event")]
pub struct CoreBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
}

// Convert the sub-behaviour events into our unified event.
impl From<gossipsub::Event> for Event {
    fn from(event: gossipsub::Event) -> Self {
        Event::Gossipsub(event)
    }
}

impl From<mdns::Event> for Event {
    fn from(event: mdns::Event) -> Self {
        Event::Mdns(event)
    }
}

// Define a trait for building our network behaviour.
pub trait NetworkBuilder {
    fn new(gossipsub: gossipsub::Behaviour, mdns: mdns::tokio::Behaviour) -> Self;
    fn get_mut_gossipsub(&mut self) -> &mut gossipsub::Behaviour;
    fn get_mut_mdns(&mut self) -> &mut mdns::tokio::Behaviour;
}

// Implement the trait for our custom behaviour.
impl NetworkBuilder for CoreBehaviour {
    fn new(gossipsub: gossipsub::Behaviour, mdns: mdns::tokio::Behaviour) -> Self {
        CoreBehaviour { gossipsub, mdns }
    }

    fn get_mut_gossipsub(&mut self) -> &mut gossipsub::Behaviour {
        &mut self.gossipsub
    }

    fn get_mut_mdns(&mut self) -> &mut mdns::tokio::Behaviour {
        &mut self.mdns
    }
}

// The main P2P struct holds our swarm.
pub struct P2P<T>
where
    T: NetworkBehaviour<ToSwarm = Event> + NetworkBuilder + Send + 'static,
{
    pub swarm: Swarm<T>,
}

impl<T> P2P<T>
where
    T: NetworkBehaviour<ToSwarm = Event> + NetworkBuilder + Send + 'static,
{
    pub fn new(swarm: Swarm<T>) -> Self {
        Self { swarm }
    }

    pub async fn serve(
        mut self,
        topic: &str,
    ) -> Result<(mpsc::Sender<String>, mpsc::Receiver<Event>), Box<dyn Error>> {
        // Create channels for publishing messages and receiving events.
        let (publish_tx, publish_rx) = mpsc::channel::<String>(32);
        let (event_tx, event_rx) = mpsc::channel::<Event>(32);

        // Create and subscribe to the Gossipsub topic.
        let topic = gossipsub::IdentTopic::new(topic);
        self.swarm
            .behaviour_mut()
            .get_mut_gossipsub()
            .subscribe(&topic)?;

        // Listen on both QUIC and TCP addresses.
        self.swarm
            .listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
        self.swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        println!("P2P node started. Use the provided channel to send messages and receive events.");

        // Spawn the event loop in a background task.
        let mut swarm = self.swarm;
        tokio::spawn(
            async move { Self::loop_input(&mut swarm, topic, publish_rx, event_tx).await },
        );

        Ok((publish_tx, event_rx))
    }

    pub fn swarm_build() -> Result<Swarm<T>, Box<dyn Error + Send + Sync>> {
        let swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_quic()
            .with_behaviour(|key| {
                // To content-address a message, we take the hash of its data and use that as its ID.
                let message_id_fn = |message: &gossipsub::Message| {
                    let mut s = DefaultHasher::new();
                    message.data.hash(&mut s);
                    gossipsub::MessageId::from(s.finish().to_string())
                };

                // Set a custom gossipsub configuration.
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .message_id_fn(message_id_fn)
                    .build()
                    .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?;

                // Build a gossipsub network behaviour.
                let gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )?;

                // Build an mDNS network behaviour.
                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id(),
                )?;
                Ok(T::new(gossipsub, mdns))
            })?
            .build();

        Ok(swarm)
    }

    pub async fn loop_input(
        swarm: &mut Swarm<T>,
        topic: gossipsub::IdentTopic,
        mut publish_rx: mpsc::Receiver<String>,
        event_tx: mpsc::Sender<Event>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        loop {
            select! {
                // Process messages received via the publish channel.
                maybe_msg = publish_rx.recv() => {
                    if let Some(msg) = maybe_msg {
                        if let Err(e) = swarm.behaviour_mut().get_mut_gossipsub().publish(topic.clone(), msg.as_bytes()) {
                            eprintln!("Publish error: {:?}", e);
                        }
                    }
                }
                // Process network events from the swarm.
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(Event::Mdns(mdns::Event::Discovered(list))) => {
                            for (peer_id, addr) in list {
                                let _ = event_tx.send(
                                    Event::Mdns(mdns::Event::Discovered(vec![(peer_id, addr)]))
                                ).await;
                                swarm.behaviour_mut().get_mut_gossipsub().add_explicit_peer(&peer_id);
                            }
                        },
                        SwarmEvent::Behaviour(Event::Mdns(mdns::Event::Expired(list))) => {
                            for (peer_id, addr) in list {
                                let _ = event_tx.send(
                                    Event::Mdns(mdns::Event::Expired(vec![(peer_id, addr)]))
                                ).await;
                                swarm.behaviour_mut().get_mut_gossipsub().remove_explicit_peer(&peer_id);
                            }
                        },
                        SwarmEvent::Behaviour(Event::Gossipsub(gossipsub::Event::Message {
                            propagation_source,
                            message_id,
                            message,
                        })) => {
                            let _ = event_tx.send(
                                Event::Gossipsub(gossipsub::Event::Message {
                                    propagation_source,
                                    message_id,
                                    message,
                                })
                            ).await;
                        },
                        SwarmEvent::NewListenAddr { address, .. } => {
                            println!("Local node is listening on {}", address);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
