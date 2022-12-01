//use libtor::{Tor, TorFlag, TorAddress, HiddenServiceVersion};
use bramble_crypto::{KeyPair, PublicKey, SecretKey};
use bramble_rendezvous::Rendezvous;
use bramble_sync::SyncProtocol;
use bramble_transport::Connection;
use libtor::{Tor, TorFlag};
use rand::{thread_rng, Rng};
use tokio::net::TcpStream;
use torcc_rs::controller::{HiddenService, TorController};

use gag::BufferRedirect;
use std::io::Read;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
pub mod test_rendezvous;
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

const CONTROL_PORT: u16 = 9151;
const SERVICE_PORT: u16 = 9150;

pub struct StormTor {
    control_port: u16,
    service_port: u16,
    started: bool,
    finished: bool,
}

pub struct RendezvousSocket {
    open: bool,
    port: u16,
}

pub struct StormRendezvous {}

impl Rendezvous for StormRendezvous {
    type Connection = TcpStream;
    type ListenFuture = Future<Result<TcpStream>>;
    type ConnectFuture = Future<Result<TcpStream>>;

    fn prepare_endpoints(&mut self, stream_key: SymmetricKey, role: Role) {}

    fn listen(&mut self) -> Self::ListenFuture {}

    fn connect(&mut self) -> Self::ConnectFuture {}
}

impl RendezvousSocket {
    pub fn read(&self, size: u16) -> &str {
        "reading"
    }
    pub fn write(&self, msg: String) -> () {}
    pub fn close(&self) -> () {}
}

struct StubStorage {}

impl StormTor {
    pub fn start() -> StormBRP {
        match Tor::new()
            .flag(TorFlag::DataDirectory("/tmp/tor-rust".into()))
            .start()
        {
            Ok(v) => println!("started tor"),
            Err(e) => println!("failed to start tor"),
        }

        StormTor {
            control_port: CONTROL_PORT,
            service_port: SERVICE_PORT,
            started: true,
            finished: false,
        }
    }

    pub fn is_running(&self) -> bool {
        self.started && !self.finished
    }

    pub fn start_background() -> StormBRP {
        let (tx, rx) = mpsc::channel();
        let (ty, ry) = mpsc::channel();
        let mut brp = StormBRP {
            control_port: CONTROL_PORT,
            service_port: SERVICE_PORT,
            started: false,
            finished: false,
        };

        let tor_monitor = thread::spawn(move || {
            let mut buf = BufferRedirect::stdout().unwrap();
            let mut tor_output = String::new();
            let tor_handle = Tor::new()
                .flag(TorFlag::DataDirectory("/tmp/tor-rust".into()))
                //.start_background().unwrap();
                .start_background();
            let mut started = false;
            for n in 1..30 {
                buf.read_to_string(&mut tor_output).unwrap();
                started = tor_output.contains("Bootstrapped 100% (done): Done");

                if started {
                    started = true;
                    tx.send(started).unwrap();
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            //if !started {
            //tx.send(started).unwrap();
            //}
            //torHandle.join();

            // do something with this value
            // maybe send the receiver into stormbrp and check it when needed
            ty.send(true).unwrap();
        });

        let started = rx.recv().unwrap();
        brp.started = started;
        brp
    }

    pub fn rendezvous(secret_key: [u8; 32], public_key: [u8; 32]) -> RendezvousSocket {
        RendezvousSocket {
            open: true,
            port: 6969,
        }
    }

    pub fn stop(&self) -> () {}
}

struct StormController {}

impl StormController {
    fn make_connection(device: KeyPair, peer: PublicKey) -> StormRendezvous {
        let (stream, sym_key, role) =
            perform_rendezvous(StormRendezvous {}, KeyPair, PublicKey).await?;
        let mut transport_stream = Connection::new(stream, sym_key, role, 98).unwrap();
        transport_stream
    }

    pub fn get_account_keys(password: &str) -> Vec<KeyPair> {
        // TODO: Replace with a storage system
        vec![KeyPair::generate(&thread_rng())]
    }

    pub fn initialize(password: &str) -> () {
        let tor = start();
        let sync = SyncProtocol {
            device: get_account_keys(password)[0],
            clients: HashSet::from([Stub {}]),
            connections: vec![],
            transport,
            storage,
        };
    }
}

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
