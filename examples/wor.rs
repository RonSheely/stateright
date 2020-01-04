//! A simple server exposing a single register that can only be written once.

extern crate clap;
extern crate serde_json;
extern crate stateright;

use clap::*;
use stateright::*;
use stateright::actor::*;
use stateright::actor::register::*;
use stateright::actor::spawn::*;
use stateright::actor::system::*;

type Value = char;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ServerState { maybe_value: Option<Value> }

struct ServerCfg;

impl<Id: Copy> Actor<Id> for ServerCfg {
    type Msg = RegisterMsg<Value, ()>;
    type State = ServerState;

    fn start(&self) -> ActorResult<Id, Self::Msg, Self::State> {
        ActorResult::start(ServerState { maybe_value: None }, |_outputs| {})
    }

    fn advance(&self, state: &Self::State, input: &ActorInput<Id, Self::Msg>) -> Option<ActorResult<Id, Self::Msg, Self::State>> {
        let ActorInput::Deliver { src, msg } = input;
        match msg {
            RegisterMsg::Put { value } if state.maybe_value.is_none() => {
                return ActorResult::advance(state, |state, _outputs| {
                    state.maybe_value = Some(*value);
                });
            }
            RegisterMsg::Get => {
                if let Some(value) = state.maybe_value {
                    return ActorResult::advance(state, |_state, outputs| {
                        outputs.send(*src, RegisterMsg::Respond { value });
                    });
                }
            }
            _ => {}
        }
        return None;
    }
}

#[cfg(test)]
#[test]
fn can_model_wor() {
    let system = ActorSystem {
        actors: vec![
            RegisterCfg::Server(ServerCfg),
            RegisterCfg::Client { server_ids: vec![0], desired_value: 'X' },
            RegisterCfg::Client { server_ids: vec![0], desired_value: 'Y' },
        ],
        init_network: Vec::new(),
        lossy_network: LossyNetwork::Yes,
    };
    let mut checker = system.checker(|_sys, state| {
        let values = response_values(&state);
        match values.as_slice() {
            [] => true,
            [v] => *v == 'X' || *v == 'Y',
            _ => false
        }
    });
    assert_eq!(checker.check(10_000), CheckResult::Pass);
    assert_eq!(checker.sources().len(), 144);
}

fn main() {
    let mut app = App::new("wor")
        .about("write-once register")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(SubCommand::with_name("check")
            .about("model check")
            .arg(Arg::with_name("client_count")
                 .help("number of clients proposing values")
                 .default_value("5")))
        .subcommand(SubCommand::with_name("spawn")
            .about("spawn with messaging over UDP"));
    let args = app.clone().get_matches();

    match args.subcommand() {
        ("check", Some(args)) => {
            let client_count = std::cmp::min(
                26, value_t!(args, "client_count", u8).expect("client_count"));
            println!("Benchmarking a write-once register with {} clients.", client_count);

            let mut actors = vec![RegisterCfg::Server(ServerCfg)];
            for i in 0..client_count {
                actors.push(RegisterCfg::Client {
                    server_ids: vec![0], desired_value: ('A' as u8 + i) as char
                });
            }

            let sys = ActorSystem { actors, init_network: Vec::new(), lossy_network: LossyNetwork::Yes };
            let mut checker = sys.checker(|_sys, state| {
                let values = response_values(&state);
                match values.as_slice() {
                    [] => true,
                    [v] => 'A' <= *v && *v <= ('A' as u8 + client_count - 1) as char,
                    _ => false
                }
            });
            checker.check_and_report();
        }
        ("spawn", Some(_args)) => {
            let port = 3000;

            println!("  This is a server written using the stateright actor library.");
            println!("  The server implements a single write-once register.");
            println!("  You can interact with the server using netcat. Example:");
            println!("$ nc -u 0 {}", port);
            println!("{}", serde_json::to_string(&RegisterMsg::Put::<char, ()> { value: 'X' }).unwrap());
            println!("{}", serde_json::to_string(&RegisterMsg::Get::<char, ()>).unwrap());
            println!();

            spawn(RegisterCfg::Server(ServerCfg), ("127.0.0.1".parse().unwrap(), port)).join().unwrap();
        }
        _ => app.print_help().unwrap(),
    }
}

