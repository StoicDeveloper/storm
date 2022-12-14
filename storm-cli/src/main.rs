// Modules needed:
// store contacts
//  - public key
//  - messages are part of sync layer
//  - so just pub key

//use async_std::prelude::*;
//use shellfish::{app, Command, Shell};
//use std::convert::TryInto;
//use std::error::Error;
//use std::fmt;
//use std::io::prelude::*;
use slice_as_array::{slice_as_array, slice_as_array_transmute};
//use std::ops::AddAssign;
//use bramble_crypto::PublicKey;
use std::sync::Mutex;
use std::{io::Write, rc::Rc};
//use std::pin::Pin;

extern crate tokio;
//use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
//use futures::channel::oneshot::Sender as OSSender;
#[allow(unused_imports)]
use futures::select;
use futures::FutureExt;
//use std::io;
use storm_backend::controller::controller::get_group;
use storm_backend::controller::controller::ControllerOutput;
#[allow(unused_imports)]
use storm_backend::controller::controller::StormController;
#[allow(unused_imports)]
use storm_backend::controller::storage::Profile;
use storm_backend::controller::tor::TorInstance;
use tokio::io::{stdout, AsyncBufReadExt, AsyncWriteExt};

enum Command {
    NewGroup(String),
    EnterGroup(String),
    ListGroups,
    ListPeers,
    Send(String),
    AddContact(String, String),
    Share(String, String),
    Login(String),
    PrintKey,
    Exit,
}

fn parse_cmd(line: &String) -> Result<Command, String> {
    use Command::*;
    //let string = ;
    //println!("{}", &string);
    match line.to_ascii_lowercase().trim().split_once(" ") {
        Some((cmd_str, arg_str)) => {
            let arg = arg_str.into();
            match cmd_str {
                "newgroup" => Ok(NewGroup(arg)),
                "entergroup" => Ok(EnterGroup(arg)),
                "send" => Ok(Send(arg)),
                "login" => Ok(Login(arg)),
                "add" => {
                    let res = arg.split_once(" ");
                    match res {
                        Some((name, key)) => Ok(AddContact(name.to_string(), key.to_string())),
                        None => Err("Must specify name and key".to_string()),
                    }
                }
                "share" => {
                    let res = arg.split_once(" ");
                    match res {
                        Some((group, name)) => Ok(Share(group.to_string(), name.to_string())),
                        None => Err("Must provide group name and peer name".to_string()),
                    }
                }
                _ => Err("Command not recognized".to_string()),
            }
        }
        None => match line.as_str().trim() {
            "groups" => Ok(ListGroups),
            "peers" => Ok(ListPeers),
            "key" => Ok(PrintKey),
            "exit" | "quit" => Ok(Exit),
            cmd => Err(format!("Invalid command: {}", cmd)),
        },
    }
}

struct InitialCliState {
    prompt: String,
    tor: Rc<Mutex<TorInstance>>,
    stdin: tokio::io::BufReader<tokio::io::Stdin>,
    stdout: tokio::io::Stdout,
}

impl InitialCliState {
    fn new() -> Self {
        //std::io::stdout().flush();
        Self {
            prompt: "Storm|>>>".to_string(),
            tor: TorInstance::new_ref(vec![]),
            stdin: tokio::io::BufReader::new(tokio::io::stdin()),
            stdout: stdout(),
        }
    }

    fn login(self, name: &str) -> LoggedInCliState {
        let profile = Profile::load(name.to_string());
        println!("{}", &format!("Loaded profile for {}.", name));
        let mut state = LoggedInCliState {
            prompt: format!("Storm|{}|>>>", name),
            controller: StormController::new(
                name,
                &format!("./data/{}.db", name),
                self.tor,
                profile.key,
            ),
            stdin: self.stdin,
            //stdout: self.stdout,
            curr_group: None,
            user: profile,
        };
        state.user.peers.iter().for_each(|(_, key)| {
            state.controller.connect_peer(*key);
        });
        state
    }

    async fn run_to_login(mut self) -> Option<LoggedInCliState> {
        loop {
            print!("{}", self.prompt);
            std::io::stdout().flush().unwrap();
            //tokio::spawn(async { stdout.flush().await? })?;
            //tokio::spawn(async { stdout.flush().await });
            let mut line = String::new();
            _ = self.stdout.flush().await;
            use Command::*;
            select!(
                _ = self.stdin.read_line(&mut line).fuse() => {
                    match parse_cmd(&line) {
                        Ok(cmd) => match &cmd {
                            Login(name) => {
                                return Some(self.login(name));
                            }
                            Exit => return None,
                            _ => println!("You muse first login before using this command"),
                        },
                        Err(e) => println!("{}", e),
                    }
                }

            )
        }
    }
}

struct LoggedInCliState {
    prompt: String,
    controller: StormController,
    stdin: tokio::io::BufReader<tokio::io::Stdin>,
    //stdout: tokio::io::Stdout,
    curr_group: Option<String>,
    user: Profile,
}

impl LoggedInCliState {
    async fn run(&mut self) {
        loop {
            print!("{}", self.prompt);
            std::io::stdout().flush().unwrap();
            //tokio::spawn(async { stdout.flush().await? })?;
            let mut line = String::new();
            use Command::*;
            select!(
                output = self.controller.run_to_output().fuse() => {
                    self.display_output(output);
                },
                _ = self.stdin.read_line(&mut line).fuse() => {
                    match parse_cmd(&line) {
                        Ok(cmd) => match &cmd {
                            Login(name) => {
                                let profile = Profile::load(name.clone());
                                let cont = StormController::new(
                                    &name,
                                    &format!("./data/{}", name),
                                    self.controller.clone_tor(),
                                    profile.key,
                                );
                                self.user = profile;
                                self.controller = cont;
                                println!("{}", &format!("Loaded profile for {}.", name));
                                break;
                            }
                            Exit => return (),
                            PrintKey => println!("{}",hex::encode(self.user.key.public())),
                            NewGroup(desc) => {
                                match self.user.groups.contains(desc) {
                                    true => print!("This group already exists"),
                                    false => {
                                        self.user.add_group(desc);
                                        self.controller.add_group(desc.to_string());
                                    }
                                }
                            }
                            EnterGroup(desc) => {
                                if self.user.groups.contains(desc) {
                                    self.curr_group = Some(desc.clone());
                                } else {
                                    println!("You are not in group {}.", desc);
                                }
                            }
                            Share(group, peer) => {
                                // do you have the peer as a contact?
                                // are you in the group?
                                // are you not already sharing the group with that peer?
                                //
                                let has_group = self.user.groups.contains(group);
                                let has_contact = self.user.peers.contains_key(peer);
                                match (has_group, has_contact) {
                                    (false, _) => print!("You do not have this group."),
                                    (_, false) => print!("You do not have this contact."),
                                    (true, true) => {
                                        let key = *self.user.peers.get(peer).unwrap();
                                        let already_sharing = self.user.peer_groups.contains(&key, group);
                                        match already_sharing {
                                            true => print!("You are already sharing this group with this contact."),
                                            false => {
                                                self.user.add_peer_to_group(key, &group);
                                                self.controller.add_peer_to_group(&key, &get_group(group).id);
                                            }
                                        }
                                    }
                                }
                            }
                            ListGroups => {
                                self.user.groups.iter().for_each(|group| println!("{}", group));
                            }
                            ListPeers => {
                                self.user.peers.iter().for_each(|(name, key)| println!("{} - {}", name, key));
                            }
                            Send(msg) => {
                                match &self.curr_group {
                                    Some(name) => {
                                        self.controller.send_group_message(name, msg);
                                    }
                                    None => println!("You must enter a group before you can send messages"),
                                }
                            }
                            AddContact(name, hex_str) => {
                                let res = hex::decode(hex_str);
                                match res {
                                    Ok(data) => {
                                        let peer_opt = slice_as_array!(&data, [u8;32]);
                                        match peer_opt {
                                            Some(peer) => {
                                                self.controller.connect_peer_slice(*peer);
                                                self.user.add_peer(&name, (*peer).into());
                                            }
                                        None =>
                                            print!("Incorrect key length"),
                                        }
                                    }
                                    Err(_) => print!("Invalid key"),
                                }
                            }
                        },
                        Err(e) => println!("{}", e),
                    }
                // if logged in, run controller and wait for the next command and the next output
                }
            )
            // else, wait for use to log in
        }
        ()
    }
    fn display_output(&self, output: ControllerOutput) {
        use storm_backend::controller::controller::Item::*;
        match &output {
            Message(msg) => {
                println!(
                    "{} - {} says: {}",
                    self.curr_group.as_ref().unwrap(),
                    msg.body.from,
                    msg.body.text
                );
            }
            Group(group) => {
                println!(
                    "New group {}",
                    String::from_utf8(group.descriptor.clone()).unwrap()
                );
            }
            Exited => {}
        }
    }
}

#[tokio::main]
async fn main() {
    if let Some(mut state) = InitialCliState::new().run_to_login().await {
        state.run().await;
    }
}
