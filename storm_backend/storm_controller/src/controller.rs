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
    tor: Rc<Mutex<TorInstance>>,
    tor_controller: Rc<Mutex<crate::tor::TorControl>>,
    key: KeyPair,
}

impl StormController {
    fn new(tor: Rc<Mutex<TorInstance>>, key: KeyPair) -> Self {
        eprintln!("making tor instance");
        block_on(tor.lock().unwrap().started());
        let tor_controller = Rc::new(Mutex::new(crate::tor::TorControl::new(
            tor.lock().unwrap().control_port,
        )));
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

#[cfg(test)]
mod test {
    use crate::tor::TorInstance;

    use super::StormController;
    use bramble_crypto::KeyPair;
    use futures::{executor::block_on, AsyncReadExt, AsyncWriteExt};
    use rand::{thread_rng, Rng};
    use tokio::join;

    fn key() -> KeyPair {
        KeyPair::generate(&mut thread_rng())
    }

    #[tokio::test]
    #[ignore]
    async fn create_controller() {
        let tor = TorInstance::new_ref(vec![]);
        StormController::new(tor, key());
    }

    #[tokio::test]
    #[ignore]
    async fn controller_create_tor_socket() {
        let tor = TorInstance::new_ref(vec![]);
        let mut cont = StormController::new(tor, key());
        cont.create_connection(*key().public()).await;
    }

    #[tokio::test]
    //#[ignore]
    async fn perform_rendezvous() {
        let key1 = key();
        let key2 = key();
        let tor = TorInstance::new_ref(vec![]);
        let mut cont1 = StormController::new(tor.clone(), key1);
        let mut cont2 = StormController::new(tor, key2);
        let (mut sock1, mut sock2) = join!(
            cont2.create_connection(*key1.public()),
            cont1.create_connection(*key2.public())
        );
        let wf1 = sock1.write_all(b"Sock1Hello");
        let mut buf1 = [0u8; 50];
        let rf1 = sock2.read(&mut buf1);
        let (_, n1) = join!(wf1, rf1);
        let res = std::str::from_utf8(&buf1[0..n1.unwrap()]).unwrap();
        eprintln!("{}", &res);
        assert_eq!("Sock1Hello".to_string(), res);

        //let wf2 = sock2.write_all(b"Sock2Greetings");
        //let mut buf2 = [0u8; 50];
        //let rf2 = sock1.read(&mut buf2);
        //let (_, n2) = join!(wf2, rf2);
        //assert_eq!(
        //"Sock2Greetings".to_string(),
        //std::str::from_utf8(&buf2[0..n2.unwrap()]).unwrap()
        //);
    }
}
