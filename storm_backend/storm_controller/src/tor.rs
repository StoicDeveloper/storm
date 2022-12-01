use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::{AsyncRead, AsyncWrite};
use gag::BufferRedirect;
use libtor::*;
use std::collections::VecDeque;
use std::future::Future;
use std::io::Read;
use std::ops::DerefMut;
use std::sync::*;
use std::task::{Poll, Waker};
use std::thread;
use std::thread::JoinHandle;

pub const CONTROL_PORT: u16 = 9151;
pub const SERVICE_PORT: u16 = 9150;
type TorHandle = JoinHandle<Result<u8, libtor::Error>>;

struct SharedTorOutputState {
    waker: Option<Waker>,
    output: VecDeque<TorOutput>,
}

// Try tracking tor state by:
// implement several futures that wait for the desired state to be reached
// Create a struct that contains the output field and a waker
// When the output field is concatenated to, wake the waker
// A future created will contain a clone of the output state,
//
// Polling just calls update state and then checks the new state
// Share SharedTorOutputState is contained in the output thread and in the tor instance struct
// The future only needs to get the waker from the SharedTorOutputState, and call it if pending

#[derive(Debug)]
enum TorOutput {
    NoChange,
    Started,
    Failed,
    Error,
    Exited,
}

impl TorOutput {
    fn process_output(line: &str) -> TorOutput {
        use TorOutput::*;
        if line.contains("Bootstrapped 100% (done): Done") {
            Started
        } else {
            NoChange
        }
    }
}

pub struct TorInstance {
    tor_monitor: JoinHandle<()>,
    output: Arc<Mutex<SharedTorOutputState>>,
    control_port: u16,
    service_port: u16,
    started: bool,
    finished: bool,
    error: bool,
}
impl TorInstance {
    fn new() -> Self {
        let output = Arc::new(Mutex::new(SharedTorOutputState {
            waker: None,
            output: VecDeque::new(),
        }));
        let output_queue = output.clone();
        let tor_monitor = thread::spawn(move || {
            let mut buf = BufferRedirect::stdout().unwrap();
            Tor::new()
                .flag(TorFlag::DataDirectory("/tmp/tor-rust".into()))
                .start_background();
            let mut output = String::new();
            eprintln!("hello world");
            loop {
                let res = buf.read_to_string(&mut output);
                match res {
                    Ok(0) => {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        continue;
                    }
                    Err(_) => {
                        eprintln!("read error");
                        break;
                    }
                    Ok(n) => {
                        eprintln!("read {} bytes", n);
                    }
                }
                let mut out = output_queue.lock().unwrap();
                for line in output.lines() {
                    eprintln!("{}", line);
                    (*out).output.push_back(TorOutput::process_output(line));
                }
                if let Some(waker) = (*out).waker.take() {
                    eprintln!("waking in loop");
                    waker.wake();
                }
                output.clear();
            }
            eprintln!("loop ended");
            let mut out = output_queue.lock().unwrap();
            if let Some(waker) = (*out).waker.take() {
                eprintln!("waking out of loop");
                waker.wake();
            }
            (*out).output.push_back(TorOutput::Exited);
        });

        println!("hello world2");
        Self {
            tor_monitor,
            output,
            control_port: CONTROL_PORT,
            service_port: SERVICE_PORT,
            started: false,
            finished: false,
            error: false,
        }
    }

    fn started(&mut self) -> TorStarting {
        TorStarting { instance: self }
    }

    fn process_output(&mut self) {
        while let Some(change) = self.output.lock().unwrap().output.pop_front() {
            let c = match &change {
                TorOutput::NoChange => {}
                TorOutput::Started => self.started = true,
                TorOutput::Failed => {
                    self.error = true;
                    self.finished = true;
                }
                TorOutput::Error => self.error = true,
                TorOutput::Exited => self.finished = true,
            };

            if !is_variant!(change, TorOutput::NoChange) {
                eprintln!("{:?}", change);
            }
        }
    }
}

pub struct TorStarting<'t> {
    instance: &'t mut TorInstance,
}

impl<'t> Future for TorStarting<'t> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.instance.process_output();
        let mut state = self.instance.output.lock().unwrap();
        println!("poll");
        eprintln!("poll");
        if self.instance.started {
            eprintln!("ready");
            Poll::Ready(())
        } else {
            // May need an array of wakers
            eprintln!("pending");
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
//fn start_background() -> mpsc::Receiver<bool> {
//let (tx, rx) = mpsc::channel();
//let (ty, ry) = mpsc::channel();
//let mut started = false;
//for n in 1..30 {
//started = tor_output.contains("Bootstrapped 100% (done): Done");

//if started {
//started = true;
//tx.send(started).unwrap();
//break;
//}
//std::thread::sleep(std::time::Duration::from_secs(1));
//}
//ty.send(started).unwrap();
//});
//}
struct TorConnection {}
impl TorConnection {}

impl AsyncWrite for TorConnection {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        todo!()
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        todo!()
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        todo!()
    }
}

impl AsyncRead for TorConnection {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use crate::tor::*;
    use futures::executor::block_on;
    use gag::BufferRedirect;
    use libtor::*;
    use std::io::Read;
    use std::sync::mpsc;
    use std::thread::{self, JoinHandle};

    fn create_tor_foreground() {
        match Tor::new()
            .flag(TorFlag::DataDirectory("/tmp/tor-rust".into()))
            .flag(TorFlag::SocksPort(19050))
            .flag(TorFlag::HiddenServiceDir("hidden_services/1".into()))
            .flag(TorFlag::HiddenServicePort(
                TorAddress::Port(8000),
                Some(TorAddress::AddressPort("127.0.0.1".into(), 5000)).into(),
            ))
            .flag(TorFlag::HiddenServiceVersion(HiddenServiceVersion::V3))
            .flag(TorFlag::HiddenServiceDir("hidden_services/2".into()))
            .flag(TorFlag::HiddenServicePort(
                TorAddress::Port(8001),
                None.into(),
            ))
            .flag(TorFlag::HiddenServiceVersion(HiddenServiceVersion::V3))
            .flag(TorFlag::HiddenServiceDir("hidden_services/3".into()))
            .flag(TorFlag::HiddenServicePort(
                TorAddress::Port(8002),
                None.into(),
            ))
            .flag(TorFlag::HiddenServiceVersion(HiddenServiceVersion::V3))
            .start()
        {
            Ok(v) => println!("started tor"),
            Err(e) => println!("failed to start tor"),
        }
    }

    #[test]
    fn create_connection() {
        //use libtor::{HiddenServiceVersion, Tor, TorAddress, TorFlag};
        let mut tor = TorInstance::new();
        block_on(tor.started());

        //create_tor_foreground();

        //println!(
        //"{} {}, {}",
        //tor_conn.control_port, tor_conn.service_port, tor_conn.started
        //);

        //let rec = start_background();
        //rec.recv().unwrap();
    }
}
