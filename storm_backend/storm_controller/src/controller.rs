//use libtor::{Tor, TorFlag, TorAddress, HiddenServiceVersion};
use crate::rendezvous::*;
use async_channel::*;
use async_select_all::SelectAll;
use bramble_common::transport::{Id, Latency};
use bramble_crypto::{KeyPair, PublicKey, Role, SecretKey};
use bramble_rendezvous::Rendezvous;
use bramble_sync::simple_client::{ClientItem, ClientMessage, MessageBody, SimpleClient};
use bramble_sync::sync::{fuse_select, Client, Group, SyncMode, SyncProtocol, ID};
use bramble_sync::*;
use bramble_transport::*;
use futures::executor::block_on;
use futures::future::LocalBoxFuture;
use futures::future::{BoxFuture, FusedFuture};
use futures::future::{Fuse, Select};
use futures::{select, AsyncRead, AsyncWrite, Future, FutureExt};
use futures_timer::Delay;
use log::{debug, error, info, trace, warn};
use std::collections::HashSet;
use std::pin::Pin;
//use bramble_sync::SyncProtocol;
//use bramble_transport::Connection;
use libtor::{Tor, TorFlag};
use torut::control::UnauthenticatedConn;
//use rand::{thread_rng, Rng};
use crate::tor::*;
use tokio::net::TcpStream;
use torcc_rs::controller::{HiddenService, TorController};

use gag::BufferRedirect;
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
//use std::error::Error;
//use ed25519::
//use std::fmt::Display

// expose start, rendezvous(), BRPSocket.write(), BRPSocket.read(), BRPSocket.close()
// use Tor and torcc-rs controller

// TODO:
// - make a tor socks connection
// - wrap a tor socks connection in a rendezvous connection
// - wrap a rendezvous connection in a transport connection
// - pass a transport connection to bramble-sync

const CONTROL_PORT: u16 = 39085;
const CONTROL_ADDR: (&'static str, u16) = ("127.0.0.1", CONTROL_PORT);
const SERVICE_PORT: u16 = 9150;
const ROOT_KEY_LABEL: &[u8] = b"org.briarproject.bramble.transport.agreement/ROOT_KEY";

pub type ControllerInput = bramble_sync::simple_client::ClientItem;
pub struct StormController {
    name: String,
    file: String,
    tor: Rc<Mutex<TorInstance>>,
    pub tor_controller: Arc<Mutex<crate::tor::TorControl>>,
    key: KeyPair,
    connector: Connector,
    stream_numbers: HashMap<PublicKey, u64>,
    //pending_connections: SelectAll<LocalBoxFuture<'static, TorSocket>>,
    client: SimpleClient,
    sync: SyncProtocol<TorSocket>,
    messages: Vec<ClientMessage>,
}

#[allow(unused_variables)]
impl StormController {
    fn new(name: &str, file: &str, tor: Rc<Mutex<TorInstance>>, key: KeyPair) -> Self {
        let tor_controller = Arc::new(Mutex::new(crate::tor::TorControl::new(
            tor.lock().unwrap().control_port,
        )));
        let (input_sender, input_receiver) = async_channel::unbounded();
        let (output_sender, output_receiver) = async_channel::unbounded();
        let client = SimpleClient::new(output_sender, input_receiver);
        let mut clients = HashMap::new();
        clients.insert(client.dyn_get_id(), input_sender);
        let sync = SyncProtocol::new(
            name.to_string(),
            file.to_string(),
            key,
            clients,
            output_receiver,
            vec![].into(),
            None,
            SyncMode::Interactive,
            4.0,
        )
        .unwrap();
        let cont = Self {
            name: name.to_string(),
            file: file.to_string(),
            tor,
            tor_controller: tor_controller.clone(),
            key,
            connector: Connector::new(name.to_string(), key, tor_controller.clone()),
            stream_numbers: HashMap::new(),
            //pending_connections: SelectAll::new(),
            client,
            sync,
            messages: vec![],
        };
        //block_on(cont.mk_auth_tor_conn());
        //cont.tor.set_control_socket();
        //cont.load_protocol_info();
        cont
    }

    fn peek_message(&mut self) -> Option<ClientMessage> {
        match self.client.pop() {
            Some(output) => {
                let msg = extract_variant!(output, ClientItem::Message);
                self.messages.push(msg.clone());
                Some(msg)
            }
            None => None,
        }
    }
    fn add_group(&mut self, name: String) -> Group {
        self.client.add_group(name)
        //bramble_sync::test::group(&name)
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
    fn get_file(&self) -> String {
        self.file.clone()
    }
    pub async fn run_to_inactive(&mut self) {
        select!(
            (peer, res) = self.connector.next_connection().fuse() => {
                let conn = res.unwrap();
                let groups = self.client.get_sharing_groups(&conn.key);
                self.sync.new_conn((conn.key, conn, groups));
            },
            _ = self.sync.sync(false).fuse() => {
                trace!("SyncProtocol inactive");
            }
            _ = self.client.run().fuse() => {
                trace!("Client exited");
            }
        )
    }

    pub fn sync_ref(&mut self) -> &mut SyncProtocol<TorSocket> {
        &mut self.sync
    }
    pub fn client_is_active(&self) -> bool {
        self.client.is_active()
    }
    pub fn send_message(&mut self, group: ID, body: MessageBody) {
        self.client
            .send_message(group, serde_json::to_string(&body).unwrap());
    }
    pub fn add_peer_to_group(&mut self, peer: &PublicKey, group: &ID) {
        self.client.add_peer_to_group(peer, group);
    }
    pub fn has_active_writers(&self) -> bool {
        self.sync.has_active_writers()
    }
    async fn run_to_output(&mut self) -> ControllerInput {
        loop {
            println!("num pending: {}", self.connector.peers.len());
            select!(
                (peer, res) = self.connector.next_connection() => {
                let conn = res.unwrap();
                    println!("found sock {:?}", &conn);
                let groups = self.client.get_sharing_groups(&conn.key);
                self.sync.new_conn((conn.key, conn, groups));
                }
                _ = self.sync.sync(false).fuse() => {
                    trace!(target: "controller","SyncProtocol inactive");
                    return ControllerInput::Exited;
                }
                output = self.client.run_to_output().fuse() => {
                    trace!(target: "controller","Client output to controller");
                    return output;
                }
            )
        }
    }

    async fn get_next_connection(&mut self) -> TorSocket {
        let (_, res) = self.connector.next_connection().await;
        res.unwrap()
    }

    async fn complete_connections(&mut self) {
        while self.has_pending_connections() {
            println!(
                "{} has {} pending connections",
                self.name,
                self.connector.peers.len()
            );
            let conn = self.get_next_connection().await;
            println!("found sock {:?}", &conn);
            let groups = self.client.get_sharing_groups(&conn.key);
            self.sync.new_conn((conn.key, conn, groups));
        }
    }

    fn has_pending_connections(&self) -> bool {
        !self.connector.peers.is_empty()
    }

    pub fn connect_peer(&mut self, peer: PublicKey) {
        //let fut = self.create_rendezvous(peer);
        self.connector.connect_peer(peer);
    }
    pub fn create_rendezvous(&mut self, peer: PublicKey) -> LocalBoxFuture<'static, TorSocket> {
        Box::pin(
            bramble_rendezvous::perform_rendezvous(
                StormRendezvous::new(self.name.to_string(), self.tor_controller.clone(), peer),
                self.key,
                peer,
            )
            .map(|res| {
                let (rendezvous_conn, _, _) = res.unwrap();
                rendezvous_conn
            }),
        )
    }

    pub fn create_transport(&mut self, rdvs: TorSocket, peer: PublicKey) -> Connection<TorSocket> {
        println!("create_transport");
        let root_key = bramble_crypto::kex(ROOT_KEY_LABEL, self.key.secret(), &peer, &[]);
        match self.stream_numbers.get_mut(&peer) {
            Some(num) => *num = *num + 1,
            None => {
                self.stream_numbers.insert(peer, 0);
            }
        }
        println!("root_key: {:?}", root_key);
        let conn = Connection::rotation(
            rdvs,
            root_key.unwrap(),
            get_role(self.key.public(), &peer),
            *self.stream_numbers.get(&peer).unwrap(),
        )
        .unwrap();
        println!("done create_transport");
        conn
    }

    //pub async fn create_storm_socket(&mut self, peer: PublicKey) -> TorSocket {
    //self.create_rendezvous(peer).await
    //}
}

// A struct that handles connecting to peers
struct Connector {
    // needs: list of connecting peers, a handle to the thread, receivers and senders to induce the
    // thread to connect to a new peer, and to get the result once a connection is made
    //handle: tokio::task::LocalSet,
    peers: HashSet<PublicKey>,
    pending: VecDeque<PublicKey>,
    sender: Sender<PublicKey>,
    receiver: Receiver<(PublicKey, Result<TorSocket>)>, // should send:
}

impl Connector {
    pub fn new(name: String, key: KeyPair, tor: Arc<Mutex<crate::tor::TorControl>>) -> Self {
        // start the worker thread
        let (key_sender, key_recv) = async_channel::unbounded::<PublicKey>();
        let (sock_sender, sock_recv) = async_channel::unbounded::<(PublicKey, Result<TorSocket>)>();
        println!("spawning connector thread for {}", &name);
        let builder = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        std::thread::spawn(move || {
            let local = tokio::task::LocalSet::new();
            local.spawn_local(async move {
                tokio::task::spawn_local(async move {
                    let device = key;
                    let mut sending = Fuse::terminated();
                    let mut send_queue: VecDeque<(PublicKey, Result<TorSocket>)> = VecDeque::new();
                    let mut peers = SelectAll::new();
                    loop {
                        println!("connector loop {}", &name);
                        select!(
                            _ = sending => {
                                if let Some(res) = send_queue.pop_back() {
                                    sending = sock_sender.send(res).fuse();
                                }
                            }
                            res = key_recv.recv().fuse() => {
                                match res {
                                    Ok(peer) => {
                                        trace!(target: "connector", "{} received key from connector",
                                            &name);
                                        let name = name.clone();
                                        let tor = tor.clone();
                                        peers.push(
                                            create_rendezvous(name, tor, device, peer)
                                        );
                                    }
                                    Err(e) =>{
                                        trace!(target: "connector", "{} key receivor has closed", &name);
                                        break;
                                    }
                                }
                            },
                            (key, res) = fuse_select(&mut peers) => {
                                if sending.is_terminated() {
                                    sending = sock_sender.send((key, res)).fuse();
                                }else{
                                    send_queue.push_back((key, res));
                                }
                            }
                        );
                    }
                });
            });
            builder.block_on(local);
        });
        Self {
            //handle,
            peers: HashSet::new(),
            pending: VecDeque::new(),
            sender: key_sender,
            receiver: sock_recv,
        }
    }
    fn next_connection(
        &mut self,
    ) -> Fuse<impl Future<Output = (PublicKey, Result<TorSocket>)> + '_> {
        if self.peers.is_empty() {
            return Fuse::terminated();
        }
        return async {
            loop {
                if self.pending.is_empty() {
                    break;
                }
                let mut sending = self.sender.send(*self.pending.front().unwrap()).fuse();
                select!(
                    _ = sending => {
                        trace!(target: "connector", "sent key to connector thread");
                        self.pending.pop_front();
                    }
                    res = self.receiver.recv().fuse() => {
                        trace!(target: "connector", "received result from connector thread");
                        let res = res.unwrap();
                        self.peers.remove(&res.0);
                        return match res {
                            (key, Ok(sock)) => (key, Ok(sock)),
                            (key, Err(e)) =>
                                (key, Err(e))
                        };
                    }
                );
            }
            let res = self.receiver.recv().await.unwrap();
            self.peers.remove(&res.0);
            return match res {
                (key, Ok(sock)) => (key, Ok(sock)),
                (key, Err(e)) => (key, Err(e)),
            };
        }
        .fuse();
    }

    pub fn connect_peer(&mut self, peer: PublicKey) {
        self.peers.insert(peer);
        self.pending.push_back(peer);
    }
}

fn create_rendezvous(
    name: String,
    tor: Arc<Mutex<crate::tor::TorControl>>,
    device: KeyPair,
    peer: PublicKey,
) -> LocalBoxFuture<'static, (PublicKey, Result<TorSocket>)> {
    trace!(target: "connector", "performing rendezvous");
    Box::pin(
        bramble_rendezvous::perform_rendezvous(
            StormRendezvous::new(name, tor.clone(), peer),
            device,
            peer,
        )
        .map(move |res| match res {
            Ok((rendezvous_conn, _, _)) => (peer, Ok(rendezvous_conn)),
            Err(e) => (peer, Err(e)),
        }),
    )
}

fn get_role(us: &PublicKey, them: &PublicKey) -> Role {
    match us.as_ref() < them.as_ref() {
        true => Role::Alice,
        false => Role::Bob,
    }
}

#[cfg(test)]
#[macro_use]
mod test {
    use bramble_sync::sync::{Group, ID};
    use futures::{select, FutureExt};
    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};
    use std::fmt;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    use std::task::Poll;

    use crate::tor::{TorInstance, TorSocket};

    use super::StormController;
    use bramble_crypto::{KeyPair, PublicKey, SymmetricKey};
    use bramble_sync::simple_client::*;
    use bramble_sync::sync::*;
    use bramble_sync::test::*;
    use futures::future::join_all;
    use futures::{executor::block_on, poll, AsyncReadExt, AsyncWriteExt};
    use rand::{thread_rng, Rng};
    use tokio::join;

    fn key() -> KeyPair {
        KeyPair::generate(&mut thread_rng())
    }

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[tokio::test]
    #[ignore]
    async fn create_controller() {
        init();
        let tor = TorInstance::new_ref(vec![]);
        let name = "Alice";
        let file = wipe(format!("./tests/{}.db", name).as_str());
        StormController::new(name, &file, tor, key());
    }

    #[tokio::test]
    #[ignore]
    async fn controller_create_tor_socket() {
        init();
        let tor = TorInstance::new_ref(vec![]);
        let name = "Alice";
        let file = wipe(format!("./tests/{}.db", name).as_str());
        let mut cont = StormController::new(name, &file, tor, key());
        cont.create_rendezvous(*key().public()).await;
    }

    #[tokio::test]
    //#[ignore]
    async fn perform_rendezvous1() {
        init();
        let key1 = key();
        let key2 = key();
        let tor = TorInstance::new_ref(vec![]);
        let name = "Alice";
        let file = wipe(format!("./tests/{}.db", name).as_str());
        let mut cont1 = StormController::new(name, &file, tor.clone(), key1);
        let name = "Bob";
        let file = wipe(format!("./tests/{}.db", name).as_str());
        let mut cont2 = StormController::new(name, &file, tor, key2);
        cont1.connect_peer(*key2.public());
        cont2.connect_peer(*key1.public());
        let (mut sock1, mut sock2) =
            join!(cont1.get_next_connection(), cont2.get_next_connection(),);
        let wf1 = sock1.write_all(b"Sock1Hello");
        let mut buf1 = [0u8; 50];
        let rf1 = sock2.read(&mut buf1);
        let (_, n1) = join!(wf1, rf1);
        let res = std::str::from_utf8(&buf1[0..n1.unwrap()]).unwrap();
        println!("{}", &res);
        assert_eq!("Sock1Hello".to_string(), res);
    }
    #[tokio::test]
    //#[ignore]
    async fn perform_rendezvous2() {
        init();
        let key1 = key();
        let key2 = key();
        let tor = TorInstance::new_ref(vec![]);
        let name = "Alice";
        let file = wipe(format!("./tests/{}.db", name).as_str());
        let mut cont1 = StormController::new(name, &file, tor.clone(), key1);
        let name = "Bob";
        let file = wipe(format!("./tests/{}.db", name).as_str());
        let mut cont2 = StormController::new(name, &file, tor, key2);
        let (mut sock1, mut sock2) = join!(
            cont1.create_rendezvous(*key2.public()),
            cont2.create_rendezvous(*key1.public())
        );
        let wf1 = sock1.write_all(b"Sock1Hello");
        //let mut buf1 = [0u8; 50];
        //let rf1 = sock2.read(&mut buf1);
        let mut buf1 = [0u8; 10];
        let rf1 = sock2.read_exact(&mut buf1);
        let (_, n1) = join!(wf1, rf1);
        //let res = std::str::from_utf8(&buf1[0..n1.unwrap()]).unwrap();
        let res = std::str::from_utf8(&buf1).unwrap();
        println!("{}", &res);
        assert_eq!("Sock1Hello".to_string(), res);
    }

    #[tokio::test]
    //#[ignore]
    async fn wrap_rendezvous_in_transport() {
        init();
        let key1 = key();
        let key2 = key();
        let tor = TorInstance::new_ref(vec![]);
        let name = "Alice";
        let file = wipe(&format!("./tests/{}.db", name).as_str());
        let mut cont1 = StormController::new(name, &file, tor.clone(), key1);
        let name = "Bob";
        let file = wipe(&format!("./tests/{}.db", name).as_str());
        let mut cont2 = StormController::new(name, &file, tor, key2);
        cont1.tor_controller.lock().unwrap().get_conf("SOCKSPort");
        cont2.tor_controller.lock().unwrap().get_conf("SOCKSPort");
        let (sock1, sock2): (TorSocket, TorSocket) = join!(
            cont1.create_rendezvous(*key2.public()),
            cont2.create_rendezvous(*key1.public())
        );
        //println!("now");
        //Delay::new(Duration::from_secs(5)).await;
        let mut sock1 = cont1.create_transport(sock1, *key2.public());
        let mut sock2 = cont2.create_transport(sock2, *key1.public());

        let fut_a = async {
            sock1.write_all(b"the quick brown fox").await.unwrap();
            sock1.flush().await.unwrap();
            sock1.write_all(b" jumps over the lazy dog").await.unwrap();
            sock1.flush().await.unwrap();
            sock1.close().await.unwrap();
        };
        let mut res = vec![];
        let fut_b = async {
            sock2.read_to_end(&mut res).await.unwrap();
        };
        block_on(async { join!(fut_a, fut_b) });
        assert_eq!(res, b"the quick brown fox jumps over the lazy dog");
    }

    fn create_one_controller(
        name: &str,
        tor: Rc<Mutex<TorInstance>>,
    ) -> (StormController, TestParams, KeyPair) {
        let file = wipe(format!("./tests/{}.db", name).as_str());
        let key = key();
        let mut controller1 = StormController::new(name, &file, tor, key);
        let p1 = storage_params(test_file(controller1.get_name()));
        (controller1, p1, key)
    }
    use linked_hash_map::LinkedHashMap;
    use linked_hash_set::LinkedHashSet;
    struct Network {
        devices: LinkedHashSet<String>,
        keys: HashMap<String, PublicKey>,
        controllers: HashMap<String, StormController>,
        stores: HashMap<String, TestParams>,
        edges: HashMap<String, HashSet<String>>,
        groups: LinkedHashMap<String, (Arc<Group>, HashSet<String>)>,
        tor: Rc<Mutex<TorInstance>>,
    }

    impl fmt::Display for Network {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(
                f,
                "Network
                Devices: {:?}
                Edges: {:?}
                Groups: {:?}",
                self.devices, self.edges, self.groups
            )
        }
    }

    impl Network {
        fn new(
            device_names: &Vec<&str>,
            edges: &Vec<(&str, &str)>,
            groups_memberships: &Vec<(&str, Vec<&str>)>,
        ) -> Self {
            let tor = TorInstance::new_ref(vec![]);
            let mut network = Self {
                devices: LinkedHashSet::new(),
                keys: HashMap::new(),
                controllers: HashMap::new(),
                stores: HashMap::new(),
                edges: HashMap::new(),
                groups: LinkedHashMap::new(),
                tor,
            };

            device_names
                .into_iter()
                .for_each(|name| network.add_device(name));
            edges.into_iter().for_each(|edge| network.add_edge(edge));
            groups_memberships.into_iter().for_each(|(group, members)| {
                network.add_group(group, members);
            });
            println!("created network");
            network
        }

        fn complete_connections(&mut self) {
            println!("complete_connections");
            block_on(futures::future::join_all(
                self.controllers
                    .values_mut()
                    .map(|cont| cont.complete_connections())
                    .collect::<Vec<_>>(),
            ));
            println!("completed connections");
        }
        fn add_device(&mut self, name: &str) {
            self.devices.insert(name.to_string());
            let (c, p, k) = create_one_controller(name, self.tor.clone());
            self.keys.insert(name.to_string(), *k.public());
            self.controllers.insert(name.to_string(), c);
            self.stores.insert(name.to_string(), p);
            self.edges.insert(name.to_string(), HashSet::new());
        }
        fn add_edge(&mut self, (peer1, peer2): &(&str, &str)) {
            let key1 = self.keys.get(*peer1).unwrap();
            let key2 = self.keys.get(*peer2).unwrap();
            self.controllers
                .get_mut(*peer1)
                .unwrap()
                .connect_peer(*key2);
            self.controllers
                .get_mut(*peer2)
                .unwrap()
                .connect_peer(*key1);
            self.edges
                .get_mut(*peer1)
                .unwrap()
                .insert((*peer2).to_string());
            self.edges
                .get_mut(*peer2)
                .unwrap()
                .insert((*peer1).to_string());
        }

        // memberships must have at least one element, there are no groups with no members
        fn add_group(&mut self, name: &str, members: &Vec<&str>) -> Arc<Group> {
            let group = group(name);
            self.groups
                .insert(name.to_string(), (group.clone(), HashSet::new()));
            members
                .iter()
                .for_each(|member| self.add_peer_to_group(name, member));
            group
        }

        fn add_peer_to_group(&mut self, name: &str, peer: &str) {
            println!("Adding peer to group in network");
            let members = &mut self.groups.get_mut(name).unwrap().1;
            println!("{:?}", members);
            let controller = self.controllers.get_mut(peer).unwrap();
            let group = block_create_group(controller, name);
            let key = self.keys.get(peer).unwrap();
            members.iter().for_each(|member| {
                let member_key = self.keys.get(member).unwrap();
                println!("adding peer to group");
                println!("{} {}", peer, member);
                controller.add_peer_to_group(member_key, &group.id)
            });
            members.iter().for_each(|member| {
                self.controllers
                    .get_mut(member)
                    .unwrap()
                    .add_peer_to_group(key, &group.id);
            });
            members.insert(peer.to_string());
        }

        fn insert_message(&mut self, peer: &str, group: ID, msg: MessageBody) {
            self.controllers
                .get_mut(peer)
                .unwrap()
                .send_message(group, msg);
        }

        fn get_group(&self, name: &str) -> Arc<Group> {
            self.groups.get(name).unwrap().0.clone()
        }

        fn run_to_all_inactive(&mut self) {
            run_network_until_inactive(self.controllers.values_mut().collect());
        }

        fn peek_msg(&mut self, peer: &str) -> ClientMessage {
            self.controllers
                .get_mut(peer)
                .unwrap()
                .peek_message()
                .unwrap()
        }

        fn group_by_id(&self, id: &ID) -> (String, Arc<Group>, HashSet<String>) {
            let (name, (group, members)) = self
                .groups
                .iter()
                .find(|(_, (group, _))| &group.id == id)
                .unwrap();
            (name.clone(), group.clone(), members.clone())
        }

        fn assert_msg_fully_shared(&self, msg: &ClientMessage) {
            use itertools::Itertools;
            let peers = self.group_by_id(&msg.group).2;
            // For every two of the above peers, check if each knows the other has the message
            peers.iter().combinations(2).for_each(|pair| {
                if self.edges.get(pair[0]).unwrap().contains(pair[1]) {
                    let k2 = self.keys.get(pair[1]).unwrap();
                    let c1 = self.stores.get(pair[0]).unwrap();
                    assert_sync_state_seen(c1, &k2, &msg.id, true);
                    let k1 = self.keys.get(pair[0]).unwrap();
                    let c2 = self.stores.get(pair[1]).unwrap();
                    assert_sync_state_seen(c2, &k1, &msg.id, true);
                }
            })

            //peers.iter().for_each(|peer| {
            //edges.iter().for_each(|(peer_a, peer_b)| {
            //let c1 = matching_controller(controllers, peer);

            //if peer_a == peer {
            //let c2 = matching_controller(controllers, peer_b);
            //assert_sync_state_seen(&c1.1, &c2.2.public(), &msg.id, true);
            //} else if peer_b == peer {
            //let c2 = matching_controller(controllers, peer_a);
            //assert_sync_state_seen(&c1.1, &c2.2.public(), &msg.id, true);
            //}
            //});
        }
    }
    fn run_network_until_inactive(mut controllers: Vec<&mut StormController>) {
        let records_to_read_counter = Rc::new(RefCell::new(0));
        controllers.iter_mut().for_each(|cont| {
            let counter = records_to_read_counter.clone();
            cont.sync_ref()
                .register_read_record_callback(Box::new(move || *counter.borrow_mut() -= 1));
            let counter = records_to_read_counter.clone();
            cont.sync_ref()
                .register_written_record_callback(Box::new(move || *counter.borrow_mut() += 1));
        });
        block_on(async {
            while *records_to_read_counter.as_ref().borrow() > 0
                || controllers
                    .iter()
                    .any(|cont| cont.has_pending_connections())
                || controllers.iter().any(|cont| cont.client_is_active())
                || controllers
                    .iter_mut()
                    .any(|cont| cont.sync_ref().is_active_writing() || cont.has_active_writers())
            {
                poll!(join_all(
                    controllers.iter_mut().map(|cont| cont.run_to_inactive())
                ));
            }
        });
    }

    #[test]
    fn test_network() {
        let devices = vec![];
        let edges = vec![];
        let groups_memberships = vec![];
        let _n = Network::new(&devices, &edges, &groups_memberships);
    }
    fn block_create_group(controller: &mut StormController, group: &str) -> Arc<Group> {
        let group = controller.add_group(group.to_string());
        let out = block_on(controller.run_to_output());
        assert_group_exists(group.id, controller.get_file().clone());
        extract_variant!(out, ClientItem::Group)
    }

    #[tokio::test]
    async fn connect_several_peers() {
        init();
        let tor = TorInstance::new_ref(vec![]);
        let (mut c1, _, k1) = create_one_controller("Peer1", tor.clone());
        let (mut c2, _, k2) = create_one_controller("Peer2", tor.clone());
        let (mut c3, _, k3) = create_one_controller("Peer3", tor.clone());
        let (mut c4, _, k4) = create_one_controller("Peer4", tor.clone());
        c1.connect_peer(*k2.public());
        c1.connect_peer(*k3.public());
        c1.connect_peer(*k4.public());
        c2.connect_peer(*k1.public());
        c2.connect_peer(*k3.public());
        c2.connect_peer(*k4.public());
        c3.connect_peer(*k1.public());
        c3.connect_peer(*k2.public());
        c3.connect_peer(*k4.public());
        c4.connect_peer(*k1.public());
        c4.connect_peer(*k2.public());
        c4.connect_peer(*k3.public());
        join!(
            c1.complete_connections(),
            c2.complete_connections(),
            c3.complete_connections(),
            c4.complete_connections(),
        );
    }

    fn block_insert_message(
        controller: &mut StormController,
        group: ID,
        msg: MessageBody,
    ) -> ClientMessage {
        controller.send_message(group, msg);
        let msg = extract_variant!(block_on(controller.run_to_output()), ClientItem::Message);
        msg
    }
    #[tokio::test]
    async fn sync_one_message() {
        init();
        let tor = TorInstance::new_ref(vec![]);
        let (mut c1, _, k1) = create_one_controller("Peer1", tor.clone());
        let (mut c2, _, k2) = create_one_controller("Peer2", tor.clone());
        c1.connect_peer(*k2.public());
        c2.connect_peer(*k1.public());
        let group = c1.add_group("Group".to_string());
        c2.add_group("Group".to_string());
        c1.add_peer_to_group(k2.public(), &group.id);
        c2.add_peer_to_group(k1.public(), &group.id);
        join!(c1.run_to_output(), c2.run_to_output());

        let msg0 = block_insert_message(&mut c1, group.id, message(None, "Blah", vec![]));
        //println!("{:?}", msg0)
        select!(_ = c1.run_to_output().fuse() => {}, _ = c2.run_to_output().fuse() => {});
    }
    #[tokio::test]
    async fn sync_one_network_message() {
        init();
        //let tor = TorInstance::new_ref(vec![]);
        let names = vec!["Alice", "Bob"];
        let edges = vec![("Alice", "Bob")];
        let memberships = vec![("Group", names.clone())];

        let mut network = Network::new(&names, &edges, &memberships);
        network.complete_connections();
        let group = network.get_group("Group").id;
        let msg0 = message(None, "Blah", vec![]);
        network.insert_message("Alice", group, msg0);
        network.run_to_all_inactive();
        let msg_a = network.peek_msg("Alice");
        network.assert_msg_fully_shared(&msg_a);
        //futures::future::join_all(
        //network
        //.controllers
        //.values_mut()
        //.map(|cont| cont.complete_connections()),
        //)
        //.await;
        //network.run_to_all_inactive();

        //select!(
        //_ = Delay::new(Duration::from_millis(1000)).fuse() => {},
        //_ = c1.run_to_exit().fuse() => {},
        //_ = c2.run_to_exit().fuse() => {},
        //);
        //let msg_a = network.peek_msg("Alice");
        //network.assert_msg_fully_shared(&msg_a);
    }

    #[tokio::test]
    async fn exchange_several_in_small_network() {
        // Test connection between 4 peers, several messages over 3 groups
        // connections:
        //      1-2,2-3,2-4
        // alternate groups of messages as so:
        //      message M_n in G_(n%|G|)
        init();
        let names = vec!["Alice", "Bob", "Charlotte", "Drake"];
        let _names_str: Vec<_> = names.iter().map(|name| (*name).to_string()).collect();
        let edges = vec![("Alice", "Bob"), ("Charlotte", "Bob"), ("Drake", "Bob")];
        let memberships = vec![("Group", names.clone())];

        //let (mut controllers, groups) = make_network(&names_str, &edges, &memberships);

        let mut network = Network::new(&names, &edges, &memberships);
        let group = network.get_group("Group").id;
        let msg0 = message(None, "Blah", vec![]);
        let msg1 = message(None, "Blag", vec![]);
        let msg2 = message(None, "Bling", vec![]);
        let msg3 = message(None, "Black", vec![]);
        network.insert_message("Alice", group, msg0);
        network.insert_message("Bob", group, msg1);
        network.insert_message("Charlotte", group, msg2);
        network.insert_message("Drake", group, msg3);

        println!("{}", &network);

        //TODO: remove the next line
        network.complete_connections();

        network.run_to_all_inactive();
        let msg_a = network.peek_msg("Alice");
        let msg_b = network.peek_msg("Bob");
        let msg_c = network.peek_msg("Charlotte");
        let msg_d = network.peek_msg("Drake");

        network.assert_msg_fully_shared(&msg_a);
        network.assert_msg_fully_shared(&msg_b);
        network.assert_msg_fully_shared(&msg_c);
        network.assert_msg_fully_shared(&msg_d);
    }
}
