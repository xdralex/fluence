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

use crate::mailbox::{
    BuiltinCommand, BuiltinCommandResult, Closure, Command, Destination, WaitResult,
};

use particle_services::Args;

use libp2p::kad::record;
use std::{sync::mpsc as std_mpsc, sync::Arc};

#[derive(Debug, Clone)]
pub struct BuiltinServicesApi {
    mailbox: Destination,
}

impl BuiltinServicesApi {
    const SERVICES: &'static [&'static str] = &["services"];

    pub fn new(mailbox: Destination) -> Self {
        Self { mailbox }
    }

    pub fn is_builtin(service_id: &str) -> bool {
        Self::SERVICES.contains(&service_id)
    }

    pub fn router(self) -> Closure {
        Arc::new(move |args| Some(Self::route(self.clone(), args).into()))
    }

    fn route(api: BuiltinServicesApi, args: Args) -> BuiltinCommandResult {
        let wait = match args.service_id.as_str() {
            "resolve" => {
                let key = args
                    .args
                    .get("key")
                    .and_then(|v| v.as_str())
                    .and_then(|s| bs58::decode(s).into_vec().ok())
                    .unwrap_or_else(|| unimplemented!("FIXME: return error?"));

                api.resolve(key.into())
            }
            "add_certificate" => unimplemented!("FIXME"),
            _ => unimplemented!("FIXME: unknown. return error? re-route to call service?"),
        };

        wait.recv().expect("receive BuiltinCommandResult")
    }

    fn resolve(&self, key: record::Key) -> WaitResult {
        let (outlet, inlet) = std_mpsc::channel();
        let cmd = Command {
            outlet,
            kind: BuiltinCommand::DHTResolve(key),
        };
        self.mailbox
            .unbounded_send(cmd)
            .expect("builtin => mailbox");

        inlet
    }
}