//! Defines an interface for register-like actors (via `RegisterMsg`) and also provides a wrapper
//! `Actor` (via `RegisterCfg`) that implements client behavior for model checking a register
//! implementation.

use crate::actor::*;
use crate::actor::system::*;
use serde_derive::Deserialize;
use serde_derive::Serialize;

/// A wrapper configuration for model-checking a register-like actor.
#[derive(Clone)]
pub enum RegisterCfg<Id, Value, ServerCfg> {
    Client {
        server_ids: Vec<Id>,
        desired_value: Value,
    },
    Server(ServerCfg),
}

/// Defines an interface for a register-like actor.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[derive(Serialize, Deserialize)]
pub enum RegisterMsg<Value, ServerMsg> {
    Put { value: Value },
    Get,
    Respond { value: Value},
    Internal(ServerMsg),
}

/// A wrapper state for model-checking a register-like actor.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RegisterState<ServerState> {
    Client,
    Server(ServerState),
}

impl<Id, Value, ServerCfg, ServerMsg: Serialize + DeserializeOwned> Actor<Id> for RegisterCfg<Id, Value, ServerCfg>
where
    Id: Copy + Ord,
    Value: Clone,
    ServerCfg: Actor<Id, Msg = RegisterMsg<Value, ServerMsg>>,
{
    type Msg = ServerCfg::Msg;
    type State = RegisterState<ServerCfg::State>;

    fn start(&self) -> ActorResult<Id, Self::Msg, Self::State> {
        match self {
            RegisterCfg::Client { ref server_ids, ref desired_value } => {
                ActorResult::start(RegisterState::Client, |outputs| {
                    for server_id in server_ids {
                        outputs.send(*server_id, RegisterMsg::Put { value: desired_value.clone() });
                        outputs.send(*server_id, RegisterMsg::Get);
                    }
                })
            }
            RegisterCfg::Server(ref server_cfg) => {
                let server_result = server_cfg.start();
                ActorResult {
                    state: RegisterState::Server(server_result.state),
                    outputs: server_result.outputs,
                }
            }
        }
    }

    fn advance(&self, state: &Self::State, input: &ActorInput<Id, Self::Msg>) -> Option<ActorResult<Id, Self::Msg, Self::State>> {
        if let RegisterCfg::Server(server_cfg) = self {
            if let RegisterState::Server(server_state) = state {
                if let Some(server_result) = server_cfg.advance(server_state, input) {
                    return Some(ActorResult {
                        state: RegisterState::Server(server_result.state),
                        outputs: server_result.outputs,
                    });
                }
            }
        }
        None
    }

    fn deserialize(&self, bytes: &[u8]) -> serde_json::Result<Self::Msg> where Self::Msg: DeserializeOwned {
        if let Ok(msg) = serde_json::from_slice::<ServerMsg>(bytes) {
            Ok(RegisterMsg::Internal(msg))
        } else {
            serde_json::from_slice(bytes)
        }
    }

    fn serialize(&self, msg: &Self::Msg) -> serde_json::Result<Vec<u8>> where Self::Msg: Serialize {
        match msg {
            RegisterMsg::Internal(msg) => serde_json::to_vec(msg),
            _ => serde_json::to_vec(msg),
        }
    }
}

/// Indicates unique values with which the server has responded.
pub fn response_values<Value: Clone + Ord, ServerMsg, ServerState>(
    state: &ActorSystemSnapshot<
        RegisterMsg<Value, ServerMsg>,
        RegisterState<ServerState>
    >) -> Vec<Value> {
    let mut values: Vec<Value> = state.network.iter().filter_map(
        |env| match &env.msg {
            RegisterMsg::Respond { value } => Some(value.clone()),
            _ => None,
        }).collect();
    values.sort();
    values.dedup();
    values
}
