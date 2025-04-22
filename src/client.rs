use std::io::{Read, Write,};
use std::net::{SocketAddr, TcpStream,};
use std::thread::{self, JoinHandle, sleep,};
use std::sync::{Arc, Mutex,};
use std::time::Duration;
use std::mem::size_of;

use crate::WorkerSize;
use crate::DataSize;

#[allow(dead_code)]
pub struct Client {
    stream_read: Arc<Mutex<TcpStream>>,
    stream_write: Arc<Mutex<TcpStream>>,
    rank: WorkerSize,
    workers_num: WorkerSize,
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
        let mut buf = [0; size_of::<WorkerSize>() * 2];
        // ランク・ワーカー数を取得
        if stream_read.lock().unwrap().read_exact(&mut buf).is_err() {
            return None;
        }

        let v = buf.chunks(size_of::<WorkerSize>()).map(|x| WorkerSize::from_be_bytes(x.try_into().unwrap())).collect::<Vec<_>>();
        Some(Self { stream_read, stream_write, rank: v[0], workers_num: v[1], })
    }
    pub fn get_rank(&self) -> WorkerSize {
        self.rank
    }
    pub fn get_workers_num(&self) -> WorkerSize {
        self.workers_num
    }
    pub fn send_to_async(&mut self, dest: WorkerSize, data: &[u8]) -> JoinHandle<()> {
        // 送信先・データサイズ・データ
        let combined_data = vec![vec![0_u8], dest.to_be_bytes().to_vec(), (data.len() as DataSize).to_be_bytes().to_vec(), data.to_vec()].concat();
        let stream = self.stream_write.clone();
        thread::spawn(move || {
            stream.lock().unwrap().write_all(&combined_data).expect("Unable to write to stream");
        })
    }
    pub fn send_to(&mut self, dest: WorkerSize, data: &[u8]) {
        // 送信先・データサイズ・データ
        let combined_data = vec![vec![0_u8], dest.to_be_bytes().to_vec(), (data.len() as DataSize).to_be_bytes().to_vec(), data.to_vec()].concat();
        self.stream_write.lock().unwrap().write_all(&combined_data).expect("Unable to write to stream");
    }
    pub fn recieve_async(&mut self) -> JoinHandle<(WorkerSize, Vec<u8>)> { // Updated return type to (WorkerSize, Vec<u8>)
        let stream = self.stream_read.clone();
        thread::spawn(move || {
            let mut stream = stream.lock().unwrap();
            let mut buf = vec![0; size_of::<WorkerSize>() + size_of::<DataSize>()];
            stream.read_exact(&mut buf).expect("Unable to read from stream");
            let source = WorkerSize::from_be_bytes(buf[..size_of::<WorkerSize>()].try_into().unwrap());
            let data_size = DataSize::from_be_bytes(buf[size_of::<WorkerSize>()..].try_into().unwrap());
            let mut buf = vec![0; data_size as usize];
            stream.read_exact(&mut buf).expect("Unable to read from stream");
            (source, buf)
        })
    }
    pub fn recieve(&mut self) -> (WorkerSize, Vec<u8>) {
        let mut buf = vec![0; size_of::<WorkerSize>() + size_of::<DataSize>()];
        let mut stream = self.stream_read.lock().unwrap();
        stream.read_exact(&mut buf).expect("Unable to read from stream");
        let source = WorkerSize::from_be_bytes(buf[..size_of::<WorkerSize>()].try_into().unwrap());
        let data_size = DataSize::from_be_bytes(buf[size_of::<WorkerSize>()..].try_into().unwrap());
        let mut buf = vec![0; data_size as usize];
        stream.read_exact(&mut buf).expect("Unable to read from stream");
        (source, buf)
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
