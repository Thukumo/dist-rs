use std::io::{Read, Write,};
use std::net::{SocketAddr, TcpStream,};
use std::thread::{self, JoinHandle, sleep,};
use std::sync::{Arc, Mutex,};
use std::time::Duration;
use std::mem::size_of;

use crate::SizeType;

#[allow(dead_code)]
pub struct Client {
    stream_read: Arc<Mutex<TcpStream>>,
    stream_write: Arc<Mutex<TcpStream>>,
    rank: SizeType,
    workers_num: SizeType,
}

#[allow(dead_code)]
impl Client {
    fn new(addr: SocketAddr, duration_ping: u64) -> Option<Self> {
        let stream = TcpStream::connect(addr).unwrap();
        let stream_read = Arc::new(Mutex::new(stream.try_clone().unwrap()));
        let stream_write = Arc::new(Mutex::new(stream));
        let stream = stream_write.clone();
        let _ = thread::spawn(move || {
            let buf = vec![255_u8];
            loop {
                sleep(Duration::from_secs(duration_ping));
                let _ = stream.lock().unwrap().write_all(&buf);
            }
        });
        let mut buf = [0; size_of::<SizeType>() * 2];
        if stream_read.lock().unwrap().read_exact(&mut buf).is_err() {
            return None;
        }

        let v = buf.chunks(size_of::<SizeType>()).map(|x| SizeType::from_be_bytes(x.try_into().unwrap())).collect::<Vec<_>>();
        Some(Self { stream_read, stream_write, rank: v[0], workers_num: v[1], })
    }
    pub fn get_rank(&self) -> SizeType {
        self.rank
    }
    pub fn get_workers_num(&self) -> SizeType {
        self.workers_num
    }
    pub fn send_to_async(&mut self, dest: SizeType, data: &[u8]) -> JoinHandle<()> {
        let data_cloned = data.to_vec(); // Clone the data to ensure ownership
        let mut data = vec![0_u8];
        data.extend(dest.to_be_bytes());
        data.extend(data_cloned.len().to_be_bytes());
        data.extend(data_cloned);
        let stream = self.stream_write.clone();
        thread::spawn(move || {
            stream.lock().unwrap().write_all(&data).expect("Unable to write to stream");
        })
    }
    pub fn send_to(&mut self, dest: SizeType, data: &[u8]) {
        let mut combined_data = vec![0_u8];
        combined_data.extend(dest.to_be_bytes());
        combined_data.extend(data.len().to_be_bytes());
        combined_data.extend(data);
        self.stream_write.lock().unwrap().write_all(&combined_data).expect("Unable to write to stream");
    }
    pub fn recieve_async(&mut self) -> JoinHandle<(SizeType, Vec<u8>)> {
        let stream = self.stream_read.clone();
        thread::spawn(move || {
            let mut stream = stream.lock().unwrap();
            let mut buf = vec![0; size_of::<SizeType>() * 2];
            stream.read_exact(&mut buf).expect("Unable to read from stream");
            let v = buf.chunks(size_of::<SizeType>()).map(|x| SizeType::from_be_bytes(x.try_into().unwrap())).collect::<Vec<_>>();
            let from = v[0];
            let mut buf = vec![0; v[1] as usize];
            stream.read_exact(&mut buf).expect("Unable to read from stream");
            (from, buf)
        })
    }
    pub fn recieve(&mut self) -> (SizeType, Vec<u8>) {
        let mut buf = vec![0; size_of::<SizeType>() * 2];
        let mut stream = self.stream_read.lock().unwrap();
        stream.read_exact(&mut buf).expect("Unable to read from stream");
        let v = buf.chunks(size_of::<SizeType>()).map(|x| SizeType::from_be_bytes(x.try_into().unwrap())).collect::<Vec<_>>();
        let from = v[0];
        let mut buf = vec![0; v[1] as usize];
        stream.read_exact(&mut buf).expect("Unable to read from stream");
        (from, buf)
    }
}
#[allow(dead_code)]
pub fn job(addr: SocketAddr, duration_ping: u64, func: fn(Client)) {
    let mut threads = Vec::new();
    for _ in 0..thread::available_parallelism().unwrap().get() {
        let func = func.clone();
        threads.push(thread::spawn(move || {
            match Client::new(addr, duration_ping) {
                Some(client) => {
                    func(client);
                },
                None => {
                }
            }
        }));
    }
    for thread in threads {
        let _ = thread.join();
    }
    println!("All workers have finished."); 
}
