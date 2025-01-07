use std::{
	collections::{hash_map, HashMap, HashSet},
	error::Error,
	time::Duration,
};

use futures::{
	channel::{mpsc, oneshot},
	prelude::*,
	StreamExt,
};
use libp2p::{
	identify, kad,
	multiaddr::Protocol,
	ping, rendezvous,
	request_response::{self, OutboundRequestId},
	swarm::{Swarm, SwarmEvent},
	Multiaddr, PeerId,
};

use crate::network::types::{Behaviour, BehaviourEvent, Command, Event, FileRequest, FileResponse};

type PendingDialResult = Result<(), Box<dyn Error + Send>>;
type PendingDialSender = oneshot::Sender<PendingDialResult>;
type FileRequestResult = Result<Vec<u8>, Box<dyn Error + Send>>;
type FileRequestSender = oneshot::Sender<FileRequestResult>;

pub(crate) struct EventLoop {
	swarm: Swarm<Behaviour>,
	command_receiver: mpsc::Receiver<Command>,
	event_sender: mpsc::Sender<Event>,
	pending_dial: HashMap<PeerId, PendingDialSender>,
	pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<()>>,
	pending_get_providers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
	pending_request_file: HashMap<OutboundRequestId, FileRequestSender>,
	cookie: Option<rendezvous::Cookie>,
	namespace: Option<rendezvous::Namespace>,
	rendezvous_point: Option<PeerId>,
}

impl EventLoop {
	pub(crate) fn new(
		swarm: Swarm<Behaviour>,
		command_receiver: mpsc::Receiver<Command>,
		event_sender: mpsc::Sender<Event>,
	) -> Self {
		Self {
			swarm,
			command_receiver,
			event_sender,
			pending_dial: Default::default(),
			pending_start_providing: Default::default(),
			pending_get_providers: Default::default(),
			pending_request_file: Default::default(),
			cookie: None,
			namespace: None,
			rendezvous_point: None,
		}
	}

	pub(crate) async fn run(mut self) {
		let mut discover_tick = tokio::time::interval(Duration::from_secs(30));

		loop {
			tokio::select! {
				event = self.swarm.select_next_some() => self.handle_event(event).await,
				command = self.command_receiver.next() => match command {
					Some(c) => self.handle_command(c).await,
					None=>  return,
				},
				_ = discover_tick.tick(), if self.rendezvous_point.is_some() => self.swarm.behaviour_mut().rendezvous.discover(
					self.namespace.clone(),
					self.cookie.clone(),
					None,
					self.rendezvous_point.unwrap(),
					),
			}
		}
	}

	async fn handle_event(&mut self, event: SwarmEvent<BehaviourEvent>) {
		match event {
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
				kad::Event::OutboundQueryProgressed {
					id,
					result: kad::QueryResult::StartProviding(_),
					..
				},
			)) => {
				let sender: oneshot::Sender<()> = self
					.pending_start_providing
					.remove(&id)
					.expect("Completed query to be previously pending.");
				let _ = sender.send(());
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
				kad::Event::OutboundQueryProgressed {
					id,
					result:
						kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
							providers,
							..
						})),
					..
				},
			)) => {
				if let Some(sender) = self.pending_get_providers.remove(&id) {
					sender.send(providers).expect("Receiver not to be dropped");

					// Finish the query. We are only interested in the first result.
					self.swarm.behaviour_mut().kademlia.query_mut(&id).unwrap().finish();
				}
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
				kad::Event::OutboundQueryProgressed {
					result:
						kad::QueryResult::GetProviders(Ok(
							kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. },
						)),
					..
				},
			)) => {},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(_)) => {},
			SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
				request_response::Event::Message { message, .. },
			)) => match message {
				request_response::Message::Request { request, channel, .. } => {
					self.event_sender
						.send(Event::InboundRequest { request: request.0, channel })
						.await
						.expect("Event receiver not to be dropped.");
				},
				request_response::Message::Response { request_id, response } => {
					let _ = self
						.pending_request_file
						.remove(&request_id)
						.expect("Request to still be pending.")
						.send(Ok(response.0));
				},
			},
			SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
				request_response::Event::OutboundFailure { request_id, error, .. },
			)) => {
				let _ = self
					.pending_request_file
					.remove(&request_id)
					.expect("Request to still be pending.")
					.send(Err(Box::new(error)));
			},
			SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
				request_response::Event::ResponseSent { .. },
			)) => {},
			SwarmEvent::NewListenAddr { address, .. } => {
				let local_peer_id = *self.swarm.local_peer_id();
				tracing::info!(
					"Listening on {}",
					address.clone().with(Protocol::P2p(local_peer_id))
				);
				eprintln!(
					"Local node is listening on {:?}",
					address.clone().with(Protocol::P2p(local_peer_id))
				);
			},
			SwarmEvent::IncomingConnection { .. } => {},
			SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
				if endpoint.is_dialer() {
					if let Some(sender) = self.pending_dial.remove(&peer_id) {
						let _ = sender.send(Ok(()));
					}
				}
				if let Err(error) = self.swarm.behaviour_mut().rendezvous.register(
					rendezvous::Namespace::from_static("rendezvous"),
					peer_id,
					None,
				) {
					tracing::error!("Failed to register: {error}");
					return;
				}
				tracing::info!("Connection established with rendezvous point {}", peer_id);
			},
			SwarmEvent::ConnectionClosed { peer_id, cause: Some(error), .. } => {
				tracing::trace!("Lost connection with {} : {}", peer_id.to_base58(), error);
			},
			SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
				if let Some(peer_id) = peer_id {
					if let Some(sender) = self.pending_dial.remove(&peer_id) {
						let _ = sender.send(Err(Box::new(error)));
					}
				}
			},
			SwarmEvent::IncomingConnectionError { .. } => {},
			SwarmEvent::Dialing { peer_id: Some(peer_id), .. } => eprintln!("Dialing {peer_id}"),
			SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Received {
				info: identify::Info { observed_addr, .. },
				..
			})) => {
				self.swarm.add_external_address(observed_addr.clone());

				tracing::info!("Received identify message from {observed_addr:?}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Rendezvous(
				rendezvous::client::Event::Discovered { registrations, cookie: new_cookie, .. },
			)) => {
				self.cookie.replace(new_cookie);

				for registration in registrations {
					for address in registration.record.addresses() {
						let peer = registration.record.peer_id();
						tracing::info!(%peer, %address, "Discovered peer");

						let p2p_suffix = Protocol::P2p(peer);
						let address_with_p2p =
							if !address.ends_with(&Multiaddr::empty().with(p2p_suffix.clone())) {
								address.clone().with(p2p_suffix)
							} else {
								address.clone()
							};

						self.swarm.dial(address_with_p2p).unwrap();
					}
				}
			},
			SwarmEvent::Behaviour(BehaviourEvent::Rendezvous(
				rendezvous::client::Event::Registered { namespace, ttl, rendezvous_node },
			)) => {
				tracing::info!(
					"Registered for namespace '{}' at rendezvous point {} for the next {} seconds",
					namespace,
					rendezvous_node,
					ttl
				);
			},
			SwarmEvent::Behaviour(BehaviourEvent::Rendezvous(
				rendezvous::client::Event::RegisterFailed { rendezvous_node, namespace, error },
			)) => {
				tracing::error!(
					"Failed to register: rendezvous_node={}, namespace={}, error_code={:?}",
					rendezvous_node,
					namespace,
					error
				);
			},
			SwarmEvent::Behaviour(BehaviourEvent::Ping(ping::Event {
				peer,
				result: Ok(rtt),
				..
			})) => {
				tracing::trace!(%peer, "Ping is {}ms", rtt.as_millis())
			},
			e => {
				tracing::warn!("Unhandled event: {:?}", e);
				// panic!("{e:?}")
			},
		}
	}

	async fn handle_command(&mut self, command: Command) {
		match command {
			Command::StartListening { addr, sender } => {
				let _ = match self.swarm.listen_on(addr) {
					Ok(_) => sender.send(Ok(())),
					Err(e) => sender.send(Err(Box::new(e))),
				};
			},
			Command::Dial { peer_id, peer_addr, sender } => {
				if let hash_map::Entry::Vacant(e) = self.pending_dial.entry(peer_id) {
					self.swarm.behaviour_mut().kademlia.add_address(&peer_id, peer_addr.clone());
					match self.swarm.dial(peer_addr.with(Protocol::P2p(peer_id))) {
						Ok(()) => {
							e.insert(sender);
						},
						Err(e) => {
							let _ = sender.send(Err(Box::new(e)));
						},
					}
				} else {
					todo!("Already dialing peer.");
				}
			},
			Command::StartProviding { file_name, sender } => {
				let query_id = self
					.swarm
					.behaviour_mut()
					.kademlia
					.start_providing(file_name.into_bytes().into())
					.expect("No store error.");
				self.pending_start_providing.insert(query_id, sender);
			},
			Command::GetProviders { file_name, sender } => {
				let query_id = self
					.swarm
					.behaviour_mut()
					.kademlia
					.get_providers(file_name.into_bytes().into());
				self.pending_get_providers.insert(query_id, sender);
			},
			Command::RequestFile { file_name, peer, sender } => {
				let request_id = self
					.swarm
					.behaviour_mut()
					.request_response
					.send_request(&peer, FileRequest(file_name));
				self.pending_request_file.insert(request_id, sender);
			},
			Command::RespondFile { file, channel } => {
				self.swarm
					.behaviour_mut()
					.request_response
					.send_response(channel, FileResponse(file))
					.expect("Connection to peer to be still open.");
			},
		}
	}
}
