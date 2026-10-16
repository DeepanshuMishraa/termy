use crate::multiplexer::{
    discovery::{self, Endpoint},
    pane::Pane,
    protocol::*,
};
use anyhow::{bail, ensure};
use fs4::fs_std::FileExt;
use rand_core::{OsRng, RngCore};
use std::{
    collections::HashMap,
    io::Write,
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

/// Connections that have not authenticated yet. Each holds a thread for up to
/// the 5 s greeting timeout; beyond this, new connections are dropped so a
/// local flood cannot exhaust threads and take down every session.
const MAX_UNAUTHENTICATED: usize = 32;
const ACCEPT_BACKOFF: Duration = Duration::from_millis(10);
const MAX_ACCEPT_BACKOFF: Duration = Duration::from_secs(1);

struct Server {
    panes: Mutex<HashMap<String, Pane>>,
    layout: Mutex<Option<String>>,
    token: String,
    address: SocketAddr,
    stopping: AtomicBool,
    unauthenticated: AtomicUsize,
}

/// Counts a connection against `MAX_UNAUTHENTICATED` until it is dropped.
struct PendingAuth(Arc<Server>);

impl PendingAuth {
    fn acquire(server: &Arc<Server>) -> Option<Self> {
        // Reserve first and give the slot back when over the cap, so a burst
        // of accepts can never push the count past it.
        if server.unauthenticated.fetch_add(1, Ordering::AcqRel) >= MAX_UNAUTHENTICATED {
            server.unauthenticated.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        Some(Self(Arc::clone(server)))
    }
}

impl Drop for PendingAuth {
    fn drop(&mut self) {
        self.0.unauthenticated.fetch_sub(1, Ordering::AcqRel);
    }
}

pub fn serve(root: &Path) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        // Launchers can pass down the inheritable "ignore Ctrl+C" attribute.
        // Clear it before creating ConPTY shells so their foreground commands
        // can be interrupted even when this host started from a background job.
        // SAFETY: a null handler changes only the current process attribute;
        // no callback or pointer is registered.
        if unsafe { windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 0) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    discovery::prepare_root(root)?;
    let lock = discovery::private_file(&root.join("host.lock"))?;
    ensure!(
        lock.try_lock_exclusive()?,
        "a session host is already running"
    );
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?;
    let mut bytes = [0; 32];
    OsRng.fill_bytes(&mut bytes);
    let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let endpoint = Endpoint {
        conditional_layout_updates: true,
        mutual_auth: true,
        graphics_stream: true,
        viewport_replies: true,
        version: VERSION,
        port: address.port(),
        token: token.clone(),
    };
    let path = root.join("endpoint.json");
    let temp = root.join("endpoint.tmp");
    let mut file = discovery::private_file(&temp)?;
    file.set_len(0)?;
    file.write_all(&serde_json::to_vec(&endpoint)?)?;
    file.sync_all()?;
    std::fs::rename(temp, &path)?;
    let server = Arc::new(Server {
        panes: Mutex::new(HashMap::new()),
        layout: Mutex::new(None),
        token,
        address,
        stopping: AtomicBool::new(false),
        unauthenticated: AtomicUsize::new(0),
    });
    accept_connections(&server, listener.incoming());
    let panes: Vec<Pane> = server
        .panes
        .lock()
        .unwrap()
        .drain()
        .map(|(_, pane)| pane)
        .collect();
    for pane in panes {
        pane.close();
    }
    std::fs::remove_file(path)?;
    Ok(())
}

/// Serves connections until shutdown. Accept and thread-spawn failures (for
/// example running out of file descriptors) are logged and retried: returning
/// an error here would exit the host and kill every session.
fn accept_connections(
    server: &Arc<Server>,
    incoming: impl Iterator<Item = std::io::Result<TcpStream>>,
) {
    let mut backoff = Duration::ZERO;
    for stream in incoming {
        if server.stopping.load(Ordering::Acquire) {
            break;
        }
        let stream = match stream {
            Ok(stream) => {
                backoff = Duration::ZERO;
                stream
            }
            Err(error) => {
                // Persistent errors return immediately; don't spin on them.
                backoff = (backoff * 2).clamp(ACCEPT_BACKOFF, MAX_ACCEPT_BACKOFF);
                log::warn!("session host failed to accept a connection: {error}");
                std::thread::sleep(backoff);
                continue;
            }
        };
        let Some(pending_auth) = PendingAuth::acquire(server) else {
            log::warn!("too many unauthenticated session connections; dropping one");
            continue;
        };
        let server = Arc::clone(server);
        let spawned = std::thread::Builder::new()
            .name("termy-session-client".into())
            .spawn(move || {
                if let Err(error) = serve_client(stream, &server, pending_auth) {
                    log::debug!("session client disconnected: {error}");
                }
            });
        if let Err(error) = spawned {
            log::warn!("session host failed to start a client thread: {error}");
        }
    }
}

impl Server {
    fn pane(&self, id: &str) -> anyhow::Result<Pane> {
        self.panes
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("terminal session no longer exists: {id}"))
    }

    fn execute(&self, request: Request) -> anyhow::Result<Response> {
        Ok(match request {
            Request::Create(launch) => {
                let pane = Pane::create(*launch)?;
                let info = pane.info();
                let state = pane.refresh()?;
                self.panes.lock().unwrap().insert(info.id.clone(), pane);
                Response::Attached(info, state)
            }
            Request::Attach(id) => {
                let pane = self.pane(&id)?;
                Response::Attached(pane.info(), pane.refresh()?)
            }
            Request::Command { pane, command } => {
                Response::Reply(self.pane(&pane)?.command(command)?)
            }
            Request::ViewportCommand { pane, command } => {
                let (changed, generation) = self.pane(&pane)?.viewport_command(command)?;
                Response::Viewport {
                    changed,
                    generation,
                }
            }
            Request::HostReply { pane, id, reply } => {
                self.pane(&pane)?.host_reply(id, reply);
                Response::Ok
            }
            Request::List => Response::Panes(
                self.panes
                    .lock()
                    .unwrap()
                    .values()
                    .map(Pane::info)
                    .collect(),
            ),
            Request::Close(id) => {
                // Closing can wait on the pane worker; don't block every other
                // client's pane lookups meanwhile.
                let pane = self.panes.lock().unwrap().remove(&id);
                if let Some(pane) = pane {
                    pane.close();
                }
                Response::Ok
            }
            Request::GetLayout => Response::Layout(self.layout.lock().unwrap().clone()),
            Request::SetLayout(layout) => {
                ensure!(layout.len() <= 4 * 1024 * 1024, "session layout too large");
                *self.layout.lock().unwrap() = Some(layout);
                Response::Ok
            }
            Request::CompareAndSetLayout {
                expected,
                replacement,
            } => {
                ensure!(
                    replacement.len() <= 4 * 1024 * 1024,
                    "session layout too large"
                );
                let mut layout = self.layout.lock().unwrap();
                let updated = *layout == expected;
                if updated {
                    *layout = Some(replacement);
                }
                drop(layout);
                Response::LayoutUpdated(updated)
            }
            Request::Shutdown
            | Request::Hello { .. }
            | Request::Authenticate { .. }
            | Request::Proof(_)
            | Request::Subscribe(_)
            | Request::SubscribeGraphics(_) => {
                bail!("invalid session request")
            }
        })
    }
}

fn serve_client(
    mut stream: TcpStream,
    server: &Server,
    pending_auth: PendingAuth,
) -> anyhow::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    authenticate(&mut stream, &server.token)?;
    write_message(&mut stream, &Response::Ok)?;
    drop(pending_auth);
    stream.set_read_timeout(None)?;
    loop {
        let request = read_message(&mut stream)?;
        if matches!(request, Request::Shutdown) {
            write_message(&mut stream, &Response::Ok)?;
            server.stopping.store(true, Ordering::Release);
            let _ = TcpStream::connect(server.address);
            return Ok(());
        }
        let graphics_stream = matches!(&request, Request::SubscribeGraphics(_));
        if let Request::Subscribe(id) | Request::SubscribeGraphics(id) = request {
            let subscription = server.pane(&id)?.subscribe()?;
            write_message(&mut stream, &Response::Ok)?;
            let mut reader = stream.try_clone()?;
            let monitor = Arc::clone(&subscription);
            std::thread::Builder::new()
                .name("termy-session-detach".into())
                .spawn(move || {
                    use std::io::Read;
                    let _ = reader.read(&mut [0u8]);
                    monitor.close();
                })?;
            let mut graphics = crate::remote::graphics::GraphicsEncoder::default();
            while let Some(mut update) = subscription.next() {
                if graphics_stream && let Update::State(state) = update {
                    let images = graphics.encode(state.graphics.as_deref().unwrap_or_default());
                    update = Update::GraphicsState(state, images);
                }
                if let Err(error) = write_message(&mut stream, &update) {
                    subscription.close();
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    return Err(error);
                }
            }
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return Ok(());
        }
        let response = server
            .execute(request)
            .unwrap_or_else(|error| Response::Error(error.to_string()));
        write_response(&mut stream, &response)?;
    }
}

fn authenticate(stream: &mut TcpStream, token: &str) -> anyhow::Result<()> {
    match read_message_limited(stream, 4096)? {
        Request::Authenticate {
            version,
            client_nonce,
        } => {
            ensure!(version == VERSION, "invalid session authentication");
            let mut server_nonce = [0; 32];
            OsRng.fill_bytes(&mut server_nonce);
            write_message(
                stream,
                &Response::Challenge {
                    server_nonce,
                    server_proof: host_proof(token, &client_nonce, &server_nonce),
                },
            )?;
            let Request::Proof(proof) = read_message_limited(stream, 4096)? else {
                bail!("session authentication required");
            };
            ensure!(
                constant_time_eq(&proof, &client_proof(token, &client_nonce, &server_nonce)),
                "invalid session authentication"
            );
        }
        // Clients from before mutual authentication send the token itself.
        Request::Hello {
            version,
            token: candidate,
        } => ensure!(
            version == VERSION && constant_time_eq(candidate.as_bytes(), token.as_bytes()),
            "invalid session authentication"
        ),
        _ => bail!("session authentication required"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::time::Instant;

    #[track_caller]
    fn wait_for_unauthenticated(server: &Server, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while server.unauthenticated.load(Ordering::Acquire) != count {
            assert!(
                Instant::now() < deadline,
                "expected {count} pending connections"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn accept_errors_and_unauthenticated_floods_do_not_stop_the_host() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = Arc::new(Server {
            panes: Mutex::new(HashMap::new()),
            layout: Mutex::new(None),
            token: "token".into(),
            address,
            stopping: AtomicBool::new(false),
            unauthenticated: AtomicUsize::new(0),
        });
        let accept_server = Arc::clone(&server);
        let host = std::thread::spawn(move || {
            let failures = (0..3).map(|_| Err(std::io::Error::other("simulated accept failure")));
            accept_connections(&accept_server, failures.chain(listener.incoming()));
        });

        // Silent connections fill every pre-authentication slot; one more is
        // dropped instead of holding another thread.
        let idle: Vec<_> = (0..MAX_UNAUTHENTICATED)
            .map(|_| TcpStream::connect(address).unwrap())
            .collect();
        wait_for_unauthenticated(&server, MAX_UNAUTHENTICATED);
        let mut extra = TcpStream::connect(address).unwrap();
        extra
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        match extra.read(&mut [0; 1]) {
            Ok(0) => {}
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
            result => panic!("connection over the limit was kept: {result:?}"),
        }
        drop(idle);
        wait_for_unauthenticated(&server, 0);

        // A wrong proof is rejected; the real client still gets in.
        let mut forged = TcpStream::connect(address).unwrap();
        let client_nonce = [1; 32];
        let greeting = Request::Authenticate {
            version: VERSION,
            client_nonce,
        };
        write_message(&mut forged, &greeting).unwrap();
        let Response::Challenge { server_nonce, .. } =
            read_message_limited(&mut forged, 4096).unwrap()
        else {
            panic!("expected a challenge");
        };
        let wrong = client_proof("wrong token", &client_nonce, &server_nonce);
        write_message(&mut forged, &Request::Proof(wrong)).unwrap();
        assert!(read_message_limited::<Response>(&mut forged, 4096).is_err());
        let mut client = TcpStream::connect(address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        discovery::authenticate(&mut client, "token").unwrap();
        assert!(matches!(
            read_message_limited(&mut client, 4096).unwrap(),
            Response::Ok
        ));
        write_message(&mut client, &Request::List).unwrap();
        assert!(
            matches!(read_message(&mut client).unwrap(), Response::Panes(panes) if panes.is_empty())
        );

        server.stopping.store(true, Ordering::Release);
        let _ = TcpStream::connect(address);
        host.join().unwrap();
    }
}
