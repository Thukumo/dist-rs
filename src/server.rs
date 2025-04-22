use std::io::{Write, Read,};
use std::net::{SocketAddr, TcpListener, TcpStream,};
use std::sync::{Arc, Mutex,};
use std::thread;
use std::mem::size_of;

use crate::WorkerSize;
use crate::DataSize;

#[allow(dead_code)]
pub struct Server {
    port: u16,
    listener_thread: Option<thread::JoinHandle<Arc<Vec<Mutex<TcpStream>>>>>,
    num_workers: WorkerSize,
}
#[allow(dead_code)]
impl Server {
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

    fn handle_client(rank: WorkerSize, num_workers: WorkerSize, clients: Arc<Vec<Mutex<TcpStream>>>) {
        let mut reader_stream = {
            let mut stream = clients[rank as usize].lock().unwrap();
            stream.write_all(&[rank.to_be_bytes(), num_workers.to_be_bytes()].concat()).unwrap();
            stream.try_clone().unwrap()
        };
        let rank_bytes = rank.to_be_bytes().to_vec();
        loop {
            let mut buf = vec![0];
            if reader_stream.read_exact(&mut buf).is_err() {
                println!("Worker {} disconnected.", rank);
                return;
            }
            if buf[0] == 255 { // ping
                continue;
            }
            let mut buf = vec![0; size_of::<WorkerSize>() + size_of::<DataSize>()];
            reader_stream.read_exact(&mut buf).unwrap();
            let dest = WorkerSize::from_be_bytes(buf[..size_of::<WorkerSize>()].try_into().unwrap());
            let data_size = DataSize::from_be_bytes(buf[size_of::<WorkerSize>()..].try_into().unwrap());

            let mut data = vec![0; data_size as usize];
            reader_stream.read_exact(&mut data).unwrap();
            let combined_data = [[rank_bytes.clone(), buf[size_of::<WorkerSize>()..].to_vec()].concat().to_vec(), data].concat();
            clients[dest as usize].lock().unwrap().write_all(&combined_data).unwrap();
            println!("Worker {} sent {} bytes to worker {}", rank, data_size, dest);
        }
    }
    pub fn get_port_num(&self) -> u16 {
        self.port
    }
}
pub fn new(addr: SocketAddr, num_workers: WorkerSize) -> Server {
    if num_workers == 0 {
        panic!("Error: num_workers must be greater than 0");
    }
    let listener = TcpListener::bind(addr).expect("Failed to bind to address");
    let port = listener.local_addr().unwrap().port();
    let listener_thread = Some(thread::spawn(move || {
        let mut streams = Vec::new();
        let mut counter: WorkerSize = 0;
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
    Server { port, listener_thread, num_workers }
}