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
	autonat, gossipsub, identify, kad, mdns,
	multiaddr::Protocol,
	ping, relay, rendezvous,
	request_response::{self, OutboundRequestId},
	swarm::{Swarm, SwarmEvent},
	upnp, Multiaddr, PeerId,
};
use tokio_util::sync::CancellationToken;

use crate::types::{Behaviour, BehaviourEvent, Command, Event, LLMRequest, LLMResponse};

type PendingDialResult = Result<(), Box<dyn Error + Send>>;
type PendingDialSender = oneshot::Sender<PendingDialResult>;
type FileRequestResult = Result<Vec<u8>, Box<dyn Error + Send>>;
type FileRequestSender = oneshot::Sender<FileRequestResult>;

static NAMESPACE: &str = "binary-souls";

pub struct EventLoop {
	swarm: Swarm<Behaviour>,
	command_receiver: mpsc::Receiver<Command>,
	event_sender: mpsc::Sender<Event>,
	pending_dial: HashMap<PeerId, PendingDialSender>,
	pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<()>>,
	pending_get_providers: HashMap<kad::QueryId, oneshot::Sender<HashSet<PeerId>>>,
	pending_request: HashMap<OutboundRequestId, FileRequestSender>,
	cookie: Option<rendezvous::Cookie>,
	namespace: Option<rendezvous::Namespace>,
	rendezvous_point: Option<PeerId>,
	rendezvous_point_address: Option<Multiaddr>,
	external_address: Option<Multiaddr>,
}

impl EventLoop {
	#[allow(clippy::too_many_arguments)]
	pub fn new(
		swarm: Swarm<Behaviour>,
		command_receiver: mpsc::Receiver<Command>,
		event_sender: mpsc::Sender<Event>,
		namespace: Option<rendezvous::Namespace>,
		rendezvous_point: Option<PeerId>,
		rendezvous_point_address: Option<Multiaddr>,
		external_address: Option<Multiaddr>,
	) -> Self {
		Self {
			swarm,
			command_receiver,
			event_sender,
			pending_dial: Default::default(),
			pending_start_providing: Default::default(),
			pending_get_providers: Default::default(),
			pending_request: Default::default(),
			cookie: None,
			namespace,
			rendezvous_point,
			rendezvous_point_address,
			external_address,
		}
	}

	fn dial_rendezvous_point_address(&mut self) {
		if let Some(rendezvous_point_address) = &self.rendezvous_point_address {
			self.swarm.dial(rendezvous_point_address.clone()).unwrap();
		}
	}

	fn register_rendezvous_point(&mut self) {
		match self.rendezvous_point {
			Some(rendezvous_point) => {
				if let Err(error) = self.swarm.behaviour_mut().rendezvous.register(
					rendezvous::Namespace::from_static(NAMESPACE),
					rendezvous_point,
					None,
				) {
					tracing::error!("Failed to register: {error}");
				} else {
					tracing::info!("Registered rendezvous point {rendezvous_point}");
				}
			},
			None => {
				tracing::trace!("No rendezvous point to register with");
			},
		}
	}

	fn add_external_address(&mut self) {
		if let Some(external_address) = &self.external_address {
			self.swarm.add_external_address(external_address.clone());
		}
	}

	pub async fn run(mut self, cancellation_token: CancellationToken) {
		let mut discover_tick = tokio::time::interval(Duration::from_secs(30));

		self.add_external_address();
		self.dial_rendezvous_point_address();
		self.register_rendezvous_point();

		loop {
			tokio::select! {
				_ = cancellation_token.cancelled() => {
					// TODO: placeholder to implement gracefully shitdown.
					break;
				},
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
			// -- Kademlia events
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
				tracing::info!("Successfully started providing");
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
					providers.clone().iter().for_each(|p| {
						tracing::info!("Found provider: {p}");
					});
					sender.send(providers).expect("Receiver not to be dropped");
					// Finish the query. We are only interested in the first result.
					self.swarm.behaviour_mut().kademlia.query_mut(&id).unwrap().finish();
				}
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
				kad::Event::OutboundQueryProgressed {
					id,
					result:
						kad::QueryResult::GetProviders(Ok(
							kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. },
						)),
					..
				},
			)) => {
				tracing::info!("No providers found for query {id}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::ModeChanged {
				new_mode,
			})) => {
				tracing::info!("Kademlia mode changed to {new_mode}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::PendingRoutablePeer {
				peer,
				address,
			})) => {
				tracing::info!("Pending routable peer: {peer} with address {address}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::RoutablePeer {
				peer,
				address,
			})) => {
				tracing::info!("Routable peer: {peer} with address {address}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::UnroutablePeer {
				peer,
			})) => {
				tracing::info!("Unroutable peer: {peer}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::RoutingUpdated {
				peer,
				addresses,
				old_peer,
				is_new_peer,
				..
			})) => {
				let addr_len = addresses.len();
				let old_peer_or_empty = old_peer.map(|p| p.to_string()).unwrap_or_default();
				tracing::info!("Routing updated for {peer} with {addr_len} addresses. Old peer: {old_peer_or_empty}. Is new peer: {is_new_peer}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::InboundRequest {
				request: kad::InboundRequest::FindNode { num_closer_peers, .. },
			})) => {
				tracing::trace!("Received FindNode request for {num_closer_peers} closer peers");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::InboundRequest {
				request:
					kad::InboundRequest::GetProvider { num_closer_peers, num_provider_peers, .. },
			})) => {
				tracing::trace!("Received GetProvider request for {num_closer_peers} closer peers and {num_provider_peers} provider peers");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::InboundRequest {
				request: kad::InboundRequest::GetRecord { num_closer_peers, present_locally },
			})) => {
				tracing::trace!("Received GetRecord request for {num_closer_peers} closer peers and {present_locally} present locally");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::InboundRequest {
				request:
					kad::InboundRequest::PutRecord {
						source,
						connection,
						record: Some(kad::Record { key, value, publisher, .. }),
					},
			})) => {
				let key_hash = sha256::digest(key.to_vec());
				let value_hash = sha256::digest(value.to_vec());
				let publisher_or_empty = publisher.map(|p| p.to_string()).unwrap_or_default();
				tracing::trace!("Received PutRecord request from {source} on connection {connection} with record (key_hash = {key_hash}, value_hash = {value_hash}, publisher = {publisher_or_empty})");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::InboundRequest {
				request:
					kad::InboundRequest::AddProvider {
						record: Some(kad::ProviderRecord { key, provider, addresses, .. }),
						..
					},
			})) => {
				let addr_len = addresses.len();
				let key_hash = sha256::digest(key.to_vec());
				tracing::trace!("Received AddProvider request for {key_hash} from {provider} with {addr_len} addresses");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Kademlia(_)) => {
				tracing::trace!("Unhandled Kademlia event");
			},

			// -- Relay events
			SwarmEvent::Behaviour(BehaviourEvent::Relay(
				relay::Event::ReservationReqAccepted { src_peer_id, renewed },
			)) => {
				tracing::trace!(
					"Reservation request accepted from {src_peer_id}. Renewed: {renewed}"
				);
			},
			SwarmEvent::Behaviour(BehaviourEvent::Relay(relay::Event::ReservationReqDenied {
				src_peer_id,
			})) => {
				tracing::trace!("Reservation request denied from {src_peer_id}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Relay(relay::Event::ReservationTimedOut {
				src_peer_id,
			})) => {
				tracing::trace!("Reservation timed out from {src_peer_id}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Relay(relay::Event::CircuitReqAccepted {
				src_peer_id,
				dst_peer_id,
			})) => {
				tracing::trace!("Circuit request accepted from {src_peer_id} to {dst_peer_id}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Relay(relay::Event::CircuitClosed {
				src_peer_id,
				dst_peer_id,
				error,
			})) => {
				let error_or_empty = error.map(|e| e.to_string()).unwrap_or_default();
				tracing::trace!(
					"Circuit closed from {src_peer_id} to {dst_peer_id}. Error: {error_or_empty}"
				);
			},

			// -- UPnP events
			SwarmEvent::Behaviour(BehaviourEvent::Upnp(upnp::Event::NewExternalAddr(addr))) => {
				tracing::info!("New external address: {addr}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Upnp(upnp::Event::ExpiredExternalAddr(addr))) => {
				tracing::info!("Expired external address: {addr}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Upnp(upnp::Event::GatewayNotFound)) => {
				tracing::info!("UPnP gateway not found");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Upnp(upnp::Event::NonRoutableGateway)) => {
				tracing::info!("UPnP gateway is not routable");
			},

			// -- auto nat events
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::InboundProbe(
				autonat::InboundProbeEvent::Request { peer, addresses, .. },
			))) => {
				let addr_len = addresses.len();
				tracing::info!("Inbound probe request for {peer} with {addr_len} addresses.");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::InboundProbe(
				autonat::InboundProbeEvent::Response { peer, address, .. },
			))) => {
				tracing::info!("Inbound probe response for {peer} with address {address}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::InboundProbe(
				autonat::InboundProbeEvent::Error {
					peer,
					error:
						autonat::InboundProbeError::InboundRequest(
							request_response::InboundFailure::Timeout,
						),
					..
				},
			))) => {
				tracing::error!("Inbound probe error for {peer}: Timeout");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::InboundProbe(
				autonat::InboundProbeEvent::Error {
					peer,
					error:
						autonat::InboundProbeError::InboundRequest(
							request_response::InboundFailure::ResponseOmission,
						),
					..
				},
			))) => {
				tracing::error!("Inbound probe error for {peer}: Response omission");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::InboundProbe(
				autonat::InboundProbeEvent::Error {
					peer,
					error:
						autonat::InboundProbeError::InboundRequest(
							request_response::InboundFailure::Io(_),
						),
					..
				},
			))) => {
				tracing::error!("Inbound probe error for {peer}: IO error");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::InboundProbe(
				autonat::InboundProbeEvent::Error {
					peer,
					error:
						autonat::InboundProbeError::InboundRequest(
							request_response::InboundFailure::UnsupportedProtocols,
						),
					..
				},
			))) => {
				tracing::error!("Inbound probe error for {peer}: Unsupported protocols");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::InboundProbe(
				autonat::InboundProbeEvent::Error {
					peer,
					error:
						autonat::InboundProbeError::InboundRequest(
							request_response::InboundFailure::ConnectionClosed,
						),
					..
				},
			))) => {
				tracing::error!("Inbound probe error for {peer}: Connection closed");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::OutboundProbe(
				autonat::OutboundProbeEvent::Request { peer, .. },
			))) => {
				tracing::info!("Outbound probe request for {peer}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::OutboundProbe(
				autonat::OutboundProbeEvent::Response { peer, address, .. },
			))) => {
				tracing::info!("Outbound probe response for {peer} with address {address}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::OutboundProbe(
				autonat::OutboundProbeEvent::Error {
					peer,
					error: autonat::OutboundProbeError::NoServer,
					..
				},
			))) => {
				let peer_or_empty = peer.map(|p| p.to_string()).unwrap_or_default();
				tracing::error!("Outbound probe error for {peer_or_empty}: No server");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::OutboundProbe(
				autonat::OutboundProbeEvent::Error {
					peer,
					error: autonat::OutboundProbeError::NoAddresses,
					..
				},
			))) => {
				let peer_or_empty = peer.map(|p| p.to_string()).unwrap_or_default();
				tracing::error!("Outbound probe error for {peer_or_empty}: No server");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::OutboundProbe(
				autonat::OutboundProbeEvent::Error {
					peer,
					error:
						autonat::OutboundProbeError::OutboundRequest(
							request_response::OutboundFailure::Timeout,
						),
					..
				},
			))) => {
				let peer_or_empty = peer.map(|p| p.to_string()).unwrap_or_default();
				tracing::error!("Outbound probe error for {peer_or_empty}: Timeout");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::OutboundProbe(
				autonat::OutboundProbeEvent::Error {
					peer,
					error:
						autonat::OutboundProbeError::OutboundRequest(
							request_response::OutboundFailure::DialFailure,
						),
					..
				},
			))) => {
				let peer_or_empty = peer.map(|p| p.to_string()).unwrap_or_default();
				tracing::error!("Outbound probe error for {peer_or_empty}: Dial failure");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::OutboundProbe(
				autonat::OutboundProbeEvent::Error {
					peer,
					error:
						autonat::OutboundProbeError::OutboundRequest(
							request_response::OutboundFailure::Io(_),
						),
					..
				},
			))) => {
				let peer_or_empty = peer.map(|p| p.to_string()).unwrap_or_default();
				tracing::error!("Outbound probe error for {peer_or_empty}: IO error");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::OutboundProbe(
				autonat::OutboundProbeEvent::Error {
					peer,
					error:
						autonat::OutboundProbeError::OutboundRequest(
							request_response::OutboundFailure::UnsupportedProtocols,
						),
					..
				},
			))) => {
				let peer_or_empty = peer.map(|p| p.to_string()).unwrap_or_default();
				tracing::error!("Outbound probe error for {peer_or_empty}: Unsupported protocols");
			},
			SwarmEvent::Behaviour(BehaviourEvent::AutoNat(autonat::Event::OutboundProbe(
				autonat::OutboundProbeEvent::Error {
					peer,
					error:
						autonat::OutboundProbeError::OutboundRequest(
							request_response::OutboundFailure::ConnectionClosed,
						),
					..
				},
			))) => {
				let peer_or_empty = peer.map(|p| p.to_string()).unwrap_or_default();
				tracing::error!("Outbound probe error for {peer_or_empty}: Connection closed");
			},

			// -- Request-Response events
			SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
				request_response::Event::Message {
					message: request_response::Message::Request { request, channel, .. },
					..
				},
			)) => {
				self.event_sender
					.send(Event::InboundRequest {
						agent_name: request.0,
						message: request.1,
						channel,
					})
					.await
					.expect("Event receiver not to be dropped.");
			},
			SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
				request_response::Event::Message {
					message: request_response::Message::Response { request_id, response },
					..
				},
			)) => {
				let _ = self
					.pending_request
					.remove(&request_id)
					.expect("Request to still be pending.")
					.send(Ok(response.0));
			},
			SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
				request_response::Event::InboundFailure { request_id, connection_id, peer, error },
			)) => {
				tracing::error!("Inbound request failed for {peer}: {error} (request_id: {request_id}, connection_id: {connection_id})");
			},
			SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
				request_response::Event::OutboundFailure { request_id, error, .. },
			)) => {
				let _ = self
					.pending_request
					.remove(&request_id)
					.expect("Request to still be pending.")
					.send(Err(Box::new(error)));
			},
			SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
				request_response::Event::ResponseSent { request_id, connection_id, peer },
			)) => {
				tracing::info!(
					"Response sent for request {request_id} on connection {connection_id} to {peer}"
				);
			},

			// -- Swarm events
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
			SwarmEvent::IncomingConnection { local_addr, send_back_addr, connection_id } => {
				tracing::info!(
					"Incoming connection from {send_back_addr} to {local_addr} with connection_id {connection_id}"
				);
			},
			SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
				if endpoint.is_dialer() {
					if let Some(sender) = self.pending_dial.remove(&peer_id) {
						let _ = sender.send(Ok(()));
					}
				}
				if let Err(error) = self.swarm.behaviour_mut().rendezvous.register(
					rendezvous::Namespace::from_static(NAMESPACE),
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
			SwarmEvent::IncomingConnectionError {
				local_addr,
				send_back_addr,
				connection_id,
				error,
			} => {
				tracing::error!(
					"Incoming connection error from {send_back_addr} to {local_addr} with connection_id {connection_id}: {error}"
				);
			},
			SwarmEvent::ExpiredListenAddr { listener_id, address } => {
				tracing::warn!("Expired listen address {address} with listener_id {listener_id}");
			},
			SwarmEvent::ListenerError { listener_id, error } => {
				tracing::warn!("Listener error with listener_id {listener_id}: {error}");
			},
			SwarmEvent::ListenerClosed { listener_id, addresses, .. } => {
				let addresses_in_string =
					addresses.iter().map(|a| a.to_string()).collect::<Vec<String>>().join(", ");
				tracing::trace!(
					"Listener closed with listener_id {listener_id} and addresses {addresses_in_string}"
				);
			},
			SwarmEvent::Dialing { peer_id: Some(peer_id), connection_id } => {
				tracing::info!("Dialing peer {peer_id} with connection_id {connection_id}");
				eprintln!("Dialing {peer_id}");
			},
			SwarmEvent::NewExternalAddrCandidate { address } => {
				tracing::trace!("New external address candidate: {address}");
			},
			SwarmEvent::ExternalAddrConfirmed { address } => {
				tracing::trace!("External address confirmed: {address}");
			},
			SwarmEvent::ExternalAddrExpired { address } => {
				tracing::trace!("External address expired: {address}");
			},
			SwarmEvent::NewExternalAddrOfPeer { peer_id, address } => {
				tracing::trace!("New external address of {peer_id}: {address}");
			},

			// -- Identify events
			SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Sent {
				peer_id,
				..
			})) => {
				tracing::info!("Sent identify info to {peer_id:?}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Received {
				info: identify::Info { observed_addr, .. },
				..
			})) => {
				self.swarm.add_external_address(observed_addr.clone());

				tracing::info!("Received identify message from {observed_addr:?}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Pushed {
				peer_id,
				connection_id,
				info: identify::Info { observed_addr, .. },
				..
			})) => {
				tracing::info!("Pushed identify info to {peer_id:?} with connection_id {connection_id} and observed_addr {observed_addr:?}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Error {
				peer_id,
				connection_id,
				error,
			})) => {
				tracing::error!(
					"Identify error with {peer_id:?} and connection_id {connection_id}: {error}"
				);
			},

			// -- Rendezvous events
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
				rendezvous::client::Event::DiscoverFailed { namespace, rendezvous_node, error },
			)) => {
				let namespace_or_empty = namespace.map(|n| n.to_string()).unwrap_or_default();
				tracing::error!(
					"Failed to discover: rendezvous_node={rendezvous_node}, namespace={namespace_or_empty}, error_code={error:#?}"
				);
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

			// -- mDNS events
			SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
				for (peer_id, _multiaddr) in list {
					tracing::trace!("mDNS discovered a new peer: {peer_id}");
					self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
				}
			},
			SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
				for (peer_id, _multiaddr) in list {
					tracing::trace!("mDNS discover peer has expired: {peer_id}");
					self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
				}
			},

			// -- Gossipsub events
			SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Message {
				propagation_source: peer_id,
				message_id: id,
				message,
			})) => {
				tracing::info!(
					"Got message: '{}' with id: {id} from peer: {peer_id}",
					String::from_utf8_lossy(&message.data),
				);
				eprintln!(
					"Got message: '{}' with id: {id} from peer: {peer_id}",
					String::from_utf8_lossy(&message.data),
				);
			},
			SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Subscribed {
				peer_id,
				topic,
			})) => {
				tracing::info!("Subscribed to topic: {topic} with peer: {peer_id}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Unsubscribed {
				peer_id,
				topic,
			})) => {
				tracing::info!("Unsubscribed from topic: {topic} with peer: {peer_id}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(
				gossipsub::Event::GossipsubNotSupported { peer_id },
			)) => {
				tracing::warn!("Gossipsub not supported by peer: {peer_id}");
			},
			SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::SlowPeer {
				peer_id,
				failed_messages,
			})) => {
				let failed_publish_messages = failed_messages.publish;
				let failed_forward_messages = failed_messages.forward;
				let failed_priority_messages = failed_messages.priority;
				let failed_non_priority_messages = failed_messages.non_priority;
				let failed_timeout_messages = failed_messages.timeout;
				tracing::warn!("Slow peer: {peer_id} with failed messages: {failed_publish_messages} publish, {failed_forward_messages} forward, {failed_priority_messages} priority, {failed_non_priority_messages} non-priority, {failed_timeout_messages} timeout");
			},

			// -- Ping events
			SwarmEvent::Behaviour(BehaviourEvent::Ping(ping::Event {
				peer,
				result: Ok(rtt),
				..
			})) => {
				tracing::trace!(%peer, "Ping is {}ms", rtt.as_millis())
			},

			// -- Unhandled events
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
			Command::StartProviding { agent_name, sender } => {
				match self
					.swarm
					.behaviour_mut()
					.kademlia
					.start_providing(agent_name.into_bytes().into())
				{
					Ok(query_id) => {
						self.pending_start_providing.insert(query_id, sender);
					},
					Err(e) => {
						tracing::error!("Failed to start providing: {:?}", e);
					},
				}
			},
			Command::GetProviders { agent_name, sender } => {
				let query_id = self
					.swarm
					.behaviour_mut()
					.kademlia
					.get_providers(agent_name.into_bytes().into());
				self.pending_get_providers.insert(query_id, sender);
			},
			Command::RequestAgent { agent_name, message, peer, sender } => {
				let request_id = self
					.swarm
					.behaviour_mut()
					.request_response
					.send_request(&peer, LLMRequest(agent_name, message));
				self.pending_request.insert(request_id, sender);
			},
			Command::RespondLLM { llm_output: file, channel } => {
				match self
					.swarm
					.behaviour_mut()
					.request_response
					.send_response(channel, LLMResponse(file))
				{
					Ok(()) => {},
					Err(e) => {
						tracing::error!("Failed to send response: {:?}", e);
					},
				}
			},
			Command::GossipMessage { topic, message } => {
				tracing::info!("About to Gossip at {topic}: {message}");
				let topic = gossipsub::IdentTopic::new(topic);
				match self.swarm.behaviour_mut().gossipsub.publish(topic, message.into_bytes()) {
					Ok(message_id) => {
						tracing::info!("Gossip done with message id: {message_id}");
					},
					Err(e) => {
						tracing::error!("Failed to gossip message: {e}");
					},
				}
			},
		}
	}
}
