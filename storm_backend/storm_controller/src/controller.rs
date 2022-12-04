//use libtor::{Tor, TorFlag, TorAddress, HiddenServiceVersion};
use crate::rendezvous::*;
use bramble_crypto::{KeyPair, PublicKey, SecretKey};
use bramble_rendezvous::Rendezvous;
use futures::executor::block_on;
use futures_timer::Delay;
//use bramble_sync::SyncProtocol;
//use bramble_transport::Connection;
use libtor::{Tor, TorFlag};
use torut::control::UnauthenticatedConn;
//use rand::{thread_rng, Rng};
use crate::tor::*;
use tokio::net::TcpStream;
use torcc_rs::controller::{HiddenService, TorController};

use gag::BufferRedirect;
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::{mpsc, Mutex};
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

pub struct StormController {
    tor: TorInstance,
    tor_controller: Rc<Mutex<crate::tor::TorControl>>,
    key: KeyPair,
}

impl StormController {
    fn new(key: KeyPair) -> Self {
        eprintln!("making tor instance");
        let mut tor = TorInstance::new(vec![]);
        block_on(tor.started());
        let tor_controller = Rc::new(Mutex::new(crate::tor::TorControl::new(tor.control_port)));
        let cont = Self {
            tor,
            tor_controller,
            key,
        };
        eprintln!("making auth tor conn");
        //block_on(cont.mk_auth_tor_conn());
        //cont.tor.set_control_socket();
        //cont.load_protocol_info();
        cont
    }

    //async fn mk_auth_tor_conn(&mut self) {
    //eprintln!("1");
    ////let stream = TcpStream::connect(CONTROL_ADDR).await.unwrap();
    //let stream = self.tor.get_control_socket().await;
    //eprintln!("1.5");
    //let mut utc = UnauthenticatedConn::new(stream);
    //eprintln!("2");
    //Delay::new(Duration::from_secs(10)).await;
    //let info = utc.load_protocol_info().await.unwrap();
    //let ad = info.make_auth_data().unwrap().unwrap();
    //eprintln!("3");
    //utc.authenticate(&ad).await.unwrap();
    //eprintln!("4");
    //self.tor_controller = Some(utc.into_authenticated().await);
    //}

    fn load_protocol_info(&mut self) {
        self.tor
            .control_socket
            .as_ref()
            .unwrap()
            .write_all(b"PROTOCOLINFO 1\r\n")
            .unwrap();
        let mut buf = [0u8; 200];
        let n = self
            .tor
            .control_socket
            .as_ref()
            .unwrap()
            .read(&mut buf)
            .unwrap();
        eprintln!("{}", std::str::from_utf8(&buf[0..n]).unwrap());
        self.tor
            .control_socket
            .as_ref()
            .unwrap()
            .write_all(b"AUTHENTICATE \r\n")
            .unwrap();
        let mut buf = [0u8; 200];
        let n = self
            .tor
            .control_socket
            .as_ref()
            .unwrap()
            .read(&mut buf)
            .unwrap();
        eprintln!("{}", std::str::from_utf8(&buf[0..n]).unwrap());
        self.tor
            .control_socket
            .as_ref()
            .unwrap()
            .write_all(b"GETINFO \r\n")
            .unwrap();
        let mut buf = [0u8; 200];
        let n = self
            .tor
            .control_socket
            .as_ref()
            .unwrap()
            .read(&mut buf)
            .unwrap();
        eprintln!("{}", std::str::from_utf8(&buf[0..n]).unwrap());
    }

    pub async fn create_connection(&mut self, peer: PublicKey) -> TorSocket {
        let (rendezvous_conn, _, _) = bramble_rendezvous::perform_rendezvous(
            StormRendezvous::new(self.tor_controller.clone()),
            self.key,
            peer,
        )
        .await
        .unwrap();
        //self.mk_auth_tor_conn().await;
        eprintln!("connection made");
        rendezvous_conn
    }
}

// TODO: DELETE
//impl RendezvousSocket {
//pub fn read(&self, size: u16) -> &str {
//"reading"
//}
//pub fn write(&self, msg: String) -> () {}
//pub fn close(&self) -> () {}
//}

// TODO: DELETE
//impl StormTor {
//pub fn start() -> StormBRP {
//match Tor::new()
//.flag(TorFlag::DataDirectory("/tmp/tor-rust".into()))
//.start()
//{
//Ok(v) => println!("started tor"),
//Err(e) => println!("failed to start tor"),
//}

//StormTor {
//control_port: CONTROL_PORT,
//service_port: SERVICE_PORT,
//started: true,
//finished: false,
//}
//}

//pub fn is_running(&self) -> bool {
//self.started && !self.finished
//}

//pub fn start_background() -> StormBRP {
//let (tx, rx) = mpsc::channel();
//let (ty, ry) = mpsc::channel();
//let mut brp = StormBRP {
//control_port: CONTROL_PORT,
//service_port: SERVICE_PORT,
//started: false,
//finished: false,
//};

//let tor_monitor = thread::spawn(move || {
//let mut buf = BufferRedirect::stdout().unwrap();
//let mut tor_output = String::new();
//let tor_handle = Tor::new()
//.flag(TorFlag::DataDirectory("/tmp/tor-rust".into()))
////.start_background().unwrap();
//.start_background();
//let mut started = false;
//for n in 1..30 {
//buf.read_to_string(&mut tor_output).unwrap();
//started = tor_output.contains("Bootstrapped 100% (done): Done");

//if started {
//started = true;
//tx.send(started).unwrap();
//break;
//}
//std::thread::sleep(std::time::Duration::from_secs(1));
//}
//if !started {
//tx.send(started).unwrap();
//}
//torHandle.join();

// do something with this value
// maybe send the receiver into stormbrp and check it when needed
//ty.send(true).unwrap();
//});

//let started = rx.recv().unwrap();
//brp.started = started;
//brp
//}

//pub fn rendezvous(secret_key: [u8; 32], public_key: [u8; 32]) -> RendezvousSocket {
//RendezvousSocket {
//open: true,
//port: 6969,
//}
//}

//pub fn stop(&self) -> () {}
//}

//impl StormController {
//fn make_connection(device: KeyPair, peer: PublicKey) -> StormRendezvous {
//let (stream, sym_key, role) =
//perform_rendezvous(StormRendezvous {}, KeyPair, PublicKey).await?;
//let mut transport_stream = Connection::new(stream, sym_key, role, 98).unwrap();
//transport_stream
//}

//pub fn get_account_keys(password: &str) -> Vec<KeyPair> {
//// TODO: Replace with a storage system
//vec![KeyPair::generate(&thread_rng())]
//}

//pub fn initialize(password: &str) -> () {
//let tor = start();
//let sync = SyncProtocol {
//device: get_account_keys(password)[0],
//clients: HashSet::from([Stub {}]),
//connections: vec![],
//transport,
//storage,
//};
//}
//}

//fn main() {
//println!("Hello, world!");
//let brp: StormBRP = StormBRP::start();
//println!("{} {}, {}", brp.control_port, brp.service_port, brp.started);

//match Tor::new()
//.flag(TorFlag::DataDirectory("/tmp/tor-rust".into()))
//.flag(TorFlag::SocksPort(19050))
//.flag(TorFlag::HiddenServiceDir("hidden_services/1".into()))
//.flag(TorFlag::HiddenServicePort(
//TorAddress::Port(8000),
//Some(TorAddress::AddressPort("127.0.0.1".into(), 5000))
//.into(),
//))
//.flag(TorFlag::HiddenServiceVersion(HiddenServiceVersion::V3))
//.flag(TorFlag::HiddenServiceDir("hidden_services/2".into()))
//.flag(TorFlag::HiddenServicePort(TorAddress::Port(8001), None.into()))
//.flag(TorFlag::HiddenServiceVersion(HiddenServiceVersion::V3))
//.flag(TorFlag::HiddenServiceDir("hidden_services/3".into()))
//.flag(TorFlag::HiddenServicePort(TorAddress::Port(8002), None.into()))
//.flag(TorFlag::HiddenServiceVersion(HiddenServiceVersion::V3))
//.start() {
//Ok(v) => println!("started tor"),
//Err(e) => println!("failed to start tor"),
//}
//}

#[cfg(test)]
mod test {
    use super::StormController;
    use bramble_crypto::KeyPair;
    use rand::{thread_rng, Rng};

    fn key() -> KeyPair {
        KeyPair::generate(&mut thread_rng())
    }

    #[tokio::test]
    #[ignore]
    async fn create_controller() {
        StormController::new(key());
    }

    #[tokio::test]
    #[ignore]
    async fn controller_create_tor_socket() {
        let mut cont = StormController::new(key());
        cont.create_connection(*key().public()).await;
    }
}
