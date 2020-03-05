/*
 * Copyright 2020 Fluence Labs Limited
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use crate::connect_protocol::events::{ToNodeEvent, ToPeerEvent};
use janus_server::{event_polling, generate_swarm_event_type};
use libp2p::{
    core::ConnectedPoint,
    core::Multiaddr,
    swarm::{NetworkBehaviour, NetworkBehaviourAction, OneShotHandler},
    PeerId,
};
use log::trace;
use std::collections::VecDeque;

type SwarmEventType = generate_swarm_event_type!(ClientConnectProtocolBehaviour);

pub struct ClientConnectProtocolBehaviour {
    /// Queue of received network messages from connected nodes
    /// that need to be handled during polling.
    events: VecDeque<SwarmEventType>,
}

impl ClientConnectProtocolBehaviour {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    pub fn send_message(&mut self, relay: PeerId, dst: PeerId, message: Vec<u8>) {
        trace!(
            "client: sending message {:?} to {:?} via relay peer {:?}",
            message,
            dst,
            relay
        );

        self.events.push_back(NetworkBehaviourAction::SendEvent {
            peer_id: relay,
            event: ToNodeEvent::Relay {
                dst_id: dst.into_bytes(),
                data: message,
            },
        })
    }
}

impl NetworkBehaviour for ClientConnectProtocolBehaviour {
    type ProtocolsHandler = OneShotHandler<ToPeerEvent, ToNodeEvent, InnerMessage>;
    type OutEvent = ToPeerEvent;

    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        Default::default()
    }

    fn addresses_of_peer(&mut self, _peer_id: &PeerId) -> Vec<Multiaddr> {
        Vec::new()
    }

    fn inject_connected(&mut self, _node_id: PeerId, _cp: ConnectedPoint) {}

    fn inject_disconnected(&mut self, _node_id: &PeerId, _cp: ConnectedPoint) {}

    fn inject_node_event(&mut self, _source: PeerId, event: InnerMessage) {
        trace!("client: new event {:?} received", event);

        match event {
            InnerMessage::Rx(m) => self
                .events
                .push_back(NetworkBehaviourAction::GenerateEvent(m)),
            InnerMessage::Tx => {}
        }
    }

    // produces ToPeerEvent events
    event_polling!(poll, events, SwarmEventType);
}

/// Transmission between the OneShotHandler message type and the InNodeMessage message type.
#[derive(Debug)]
pub enum InnerMessage {
    /// Message has been received from a remote.
    Rx(ToPeerEvent),

    /// RelayMessage has been sent
    Tx,
}

impl From<ToPeerEvent> for InnerMessage {
    #[inline]
    fn from(in_message: ToPeerEvent) -> InnerMessage {
        InnerMessage::Rx(in_message)
    }
}

impl From<()> for InnerMessage {
    #[inline]
    fn from(_: ()) -> InnerMessage {
        InnerMessage::Tx
    }
}
