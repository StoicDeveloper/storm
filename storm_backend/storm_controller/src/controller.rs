//use libtor::{Tor, TorFlag, TorAddress, HiddenServiceVersion};
use crate::rendezvous::*;
use bramble_crypto::{KeyPair, PublicKey, Role, SecretKey};
use bramble_rendezvous::Rendezvous;
use bramble_transport::*;
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
use std::collections::HashMap;
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
const ROOT_KEY_LABEL: &[u8] = b"org.briarproject.bramble.transport.agreement/ROOT_KEY";

pub struct StormController {
    name: String,
    tor: Rc<Mutex<TorInstance>>,
    pub tor_controller: Rc<Mutex<crate::tor::TorControl>>,
    key: KeyPair,
    stream_numbers: HashMap<PublicKey, u64>,
}

impl StormController {
    fn new(name: &str, tor: Rc<Mutex<TorInstance>>, key: KeyPair) -> Self {
        let tor_controller = Rc::new(Mutex::new(crate::tor::TorControl::new(
            tor.lock().unwrap().control_port,
        )));
        let cont = Self {
            name: name.to_string(),
            tor,
            tor_controller,
            key,
            stream_numbers: HashMap::new(),
        };
        //block_on(cont.mk_auth_tor_conn());
        //cont.tor.set_control_socket();
        //cont.load_protocol_info();
        cont
    }

    pub async fn create_rendezvous(&mut self, peer: PublicKey) -> TorSocket {
        let (rendezvous_conn, _, _) = bramble_rendezvous::perform_rendezvous(
            StormRendezvous::new(&self.name, self.tor_controller.clone()),
            self.key,
            peer,
        )
        .await
        .unwrap();
        //self.mk_auth_tor_conn().await;
        rendezvous_conn
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
}

fn get_role(us: &PublicKey, them: &PublicKey) -> Role {
    match us.as_ref() < them.as_ref() {
        true => Role::Alice,
        false => Role::Bob,
    }
}

#[cfg(test)]
mod test {
    use std::task::Poll;

    use crate::tor::TorInstance;

    use super::StormController;
    use bramble_crypto::{KeyPair, SymmetricKey};
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
        StormController::new("Alice", tor, key());
    }

    #[tokio::test]
    #[ignore]
    async fn controller_create_tor_socket() {
        init();
        let tor = TorInstance::new_ref(vec![]);
        let mut cont = StormController::new("Alice", tor, key());
        cont.create_rendezvous(*key().public()).await;
    }

    #[tokio::test]
    //#[ignore]
    async fn perform_rendezvous1() {
        init();
        let key1 = key();
        let key2 = key();
        let tor = TorInstance::new_ref(vec![]);
        let mut cont1 = StormController::new("Alice", tor.clone(), key1);
        let mut cont2 = StormController::new("Bob", tor, key2);
        let (mut sock1, mut sock2) = join!(
            cont2.create_rendezvous(*key1.public()),
            cont1.create_rendezvous(*key2.public())
        );
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
        let mut cont1 = StormController::new("Alice", tor.clone(), key1);
        let mut cont2 = StormController::new("Bob", tor, key2);
        let (mut sock1, mut sock2) = join!(
            cont1.create_rendezvous(*key2.public()),
            cont2.create_rendezvous(*key1.public())
        );
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
    async fn wrap_rendezvous_in_transport() {
        init();
        let key1 = key();
        let key2 = key();
        let tor = TorInstance::new_ref(vec![]);
        let mut cont1 = StormController::new("Alice", tor.clone(), key1);
        let mut cont2 = StormController::new("Bob", tor, key2);
        cont1.tor_controller.lock().unwrap().get_conf("SOCKSPort");
        cont2.tor_controller.lock().unwrap().get_conf("SOCKSPort");
        let (sock1, sock2) = join!(
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

    use super::*;
    use bramble_common::make_duplex;

    #[test]
    fn writer_and_reader_work_together() {
        init();
        let rng = &mut thread_rng();
        let root_key = SymmetricKey::generate(rng);
        let (transport_a, transport_b) = make_duplex();
        let mut stream_a = Connection::rotation(transport_a, root_key, Role::Alice, 17).unwrap();
        let mut stream_b = Connection::rotation(transport_b, root_key, Role::Bob, 23).unwrap();

        let fut_a = async {
            use futures::io::AsyncWriteExt;
            stream_a.write_all(b"the quick brown fox").await.unwrap();
            stream_a.flush().await.unwrap();
            stream_a
                .write_all(b" jumps over the lazy dog")
                .await
                .unwrap();
            stream_a.flush().await.unwrap();
            stream_a.close().await.unwrap();
        };
        let mut res = vec![];
        let fut_b = async {
            use futures::io::AsyncReadExt;
            stream_b.read(&mut res).await.unwrap();
        };
        block_on(async { join!(fut_a, fut_b) });
        assert_eq!(res, b"the quick brown fox jumps over the lazy dog");
    }
}
