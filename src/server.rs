use std::net::UdpSocket;
use std::io::{Write, Read,};
use std::net::{SocketAddr, TcpListener, TcpStream,};
use std::sync::{Arc, Mutex,};
use std::thread;
use std::mem::size_of;

pub type SizeType = u64;
#[allow(dead_code)]
pub struct Server {
    local_addr: SocketAddr,
    listener_thread: Option<thread::JoinHandle<Arc<Vec<Mutex<TcpStream>>>>>,
    num_workers: SizeType,
}
#[allow(dead_code)]
impl Server {
    pub fn new(addr: SocketAddr, num_workers: SizeType) -> Self {
        if num_workers == 0 {
            panic!("Error: num_workers must be greater than 0");
        }
        let listener = TcpListener::bind(addr).expect("Failed to bind to address");
        let mut local_addr = listener.local_addr().unwrap();
        local_addr.set_ip({
            let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
            socket.connect("8.8.8.8:80").unwrap();
            socket.local_addr().unwrap().ip()
        });
        let listener_thread = Some(thread::spawn(move || {
            let mut streams = Vec::new();
            let mut counter: SizeType = 0;
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        streams.push(Mutex::new(stream));
                        counter += 1;
                        if num_workers == counter {
                            println!("All workers are connected.");
                            break;
                        }
                    }
                    Err(_) => {
                    }
                }
            }
            Arc::new(streams)
        }));
        Self { local_addr, listener_thread, num_workers }
    }
    pub fn run(&mut self) {
        let mut clients = Vec::new();
        let streams = self.listener_thread.take().unwrap().join().unwrap();
        for i in 0..self.num_workers {
            let streams_clone = streams.clone();
            let num_workers = self.num_workers;
            clients.push(thread::spawn(move || {
                Server::handle_client(i, num_workers, streams_clone);
            }));
        }
        for client in clients {
            if let Err(e) = client.join() {
                eprintln!("Error joining thread: {:?}", e);
            }
        }
        println!("Server shutting down.");
    }

    fn handle_client(rank: SizeType, num_workers: SizeType, clients: Arc<Vec<Mutex<TcpStream>>>) {
        let mut reader_stream = {
            let mut stream = clients[rank as usize].lock().unwrap();
            stream.write_all(&[rank.to_be_bytes(), num_workers.to_be_bytes()].concat()).unwrap();
            stream.try_clone().unwrap()
        };
        let rank_bytes = rank.to_be_bytes();
        loop {
            let mut buf = vec![0];
            if reader_stream.read_exact(&mut buf).is_err() {
                println!("Worker {} disconnected.", rank);
                return;
            }
            if buf[0] == 255 { // ping
                continue;
            }
            let mut buf = vec![0; size_of::<SizeType>() * 2];
            reader_stream.read_exact(&mut buf).unwrap();
            let description = buf.chunks(size_of::<SizeType>())
                .map(|x| SizeType::from_be_bytes(x.try_into().unwrap())).collect::<Vec<_>>();
            let mut data = vec![0; description[1] as usize];
            reader_stream.read_exact(&mut data).unwrap();
            let combined_data = [[rank_bytes, description[1].to_be_bytes()].concat().to_vec(), data].concat();
            clients[description[0] as usize].lock().unwrap().write_all(&combined_data).unwrap();
            println!("Worker {} sent {} bytes to worker {}", rank, description[1], description[0]);
        }
    }
    pub fn get_local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}
