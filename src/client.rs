// client.rs
use rand::{thread_rng, Rng};
use raptorq::{Encoder, ObjectTransmissionInformation};
use std::{
    error::Error,
    net::SocketAddr,
    time::{Duration, Instant},
};
use udplite::UdpLiteSocket;

const MIN_PACKETS: u32 = 5;
const MAX_PACKETS: u32 = 20;
const DATA_SIZE: u64 = 1300;
const MIN_SYMBOL_SIZE: u16 = 500;
const MAX_SYMBOL_SIZE: u16 = 2000;
const TIMEOUT_MS: u64 = 1000;
const NETWORK_QUALITY_WINDOW: usize = 10;

struct NetworkStats {
    packet_loss_rate: f64,
    latencies: Vec<u32>,
}

impl NetworkStats {
    fn new() -> Self {
        NetworkStats {
            packet_loss_rate: 0.0,
            latencies: Vec::with_capacity(NETWORK_QUALITY_WINDOW),
        }
    }

    fn update(&mut self, packet_received: bool, latency: Option<u128>) {
        self.packet_loss_rate = 0.9 * self.packet_loss_rate + 0.1 * (!packet_received as u8 as f64);
        if let Some(lat) = latency {
            if self.latencies.len() >= NETWORK_QUALITY_WINDOW {
                self.latencies.remove(0);
            }
            self.latencies.push(lat as u32);
        }
    }

    fn get_network_quality(&self) -> f64 {
        if self.latencies.is_empty() {
            return 0.5; // Default to middle quality if no data
        }
        let avg_latency = self.latencies.iter().sum::<u32>() as f64 / self.latencies.len() as f64;
        let normalized_latency = 1.0 / (1.0 + avg_latency / 1000.0);
        let packet_success_rate = 1.0 - self.packet_loss_rate;
        (normalized_latency + packet_success_rate) / 2.0
    }
}

fn calculate_symbol_size(network_quality: f64) -> u16 {
    let size = (MIN_SYMBOL_SIZE as f64
        + (MAX_SYMBOL_SIZE - MIN_SYMBOL_SIZE) as f64 * network_quality) as u16;
    let rounded_size = (size + 1) & !1; // Round to the nearest even number
    rounded_size.clamp(MIN_SYMBOL_SIZE, MAX_SYMBOL_SIZE)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let server_addr: SocketAddr = "127.0.0.1:55555".parse()?;
    let socket = UdpLiteSocket::bind("0.0.0.0:0")?;
    socket.set_send_checksum_coverage(Some(8))?;
    socket.set_read_timeout(Some(Duration::from_millis(TIMEOUT_MS)))?;

    println!("Client connected to server at: {}", server_addr);

    let mut network_stats = NetworkStats::new();
    let mut packets_to_send = MIN_PACKETS;
    let mut consecutive_successes = 0;
    let mut consecutive_failures = 0;

    let mut symbol_size = MIN_SYMBOL_SIZE;
    loop {
        let network_quality = network_stats.get_network_quality();
        let calculated_symbol_size = calculate_symbol_size(network_quality);

        println!(
            "Starting new transmission with {} packets, symbol size: {} (calculated: {})",
            packets_to_send, symbol_size, calculated_symbol_size
        );

        let mut data = vec![0u8; DATA_SIZE as usize];
        thread_rng().fill(&mut data[..]);

        let oti = ObjectTransmissionInformation::with_defaults(DATA_SIZE, symbol_size);
        let encoder = Encoder::new(&data, oti);
        let packets = encoder.get_encoded_packets(packets_to_send);

        let start_time = Instant::now();
        let mut pong_received = false;

        for (i, packet) in packets.into_iter().enumerate() {
            let serialized = packet.serialize();
            let serialized_len = serialized.len();
            socket.send_to(&serialized, server_addr)?;
            println!(
                "Packet {}/{} sent with {} bytes",
                i + 1,
                packets_to_send,
                serialized_len
            );

            if i == packets_to_send as usize - 1 {
                let mut buf = vec![0u8; 20];
                match socket.recv_from(&mut buf) {
                    Ok((size, _)) => {
                        let pong_msg = String::from_utf8_lossy(&buf[..size]);
                        if pong_msg.starts_with("Meow:") {
                            pong_received = true;
                            let elapsed = start_time.elapsed();
                            println!("Received pong in {}ms", elapsed.as_millis());
                            network_stats.update(true, Some(elapsed.as_millis()));

                            if let Some(new_symbol_size) =
                                pong_msg.split(':').nth(1).and_then(|s| s.parse().ok())
                            {
                                println!(
                                    "Received new symbol size: {} (current: {})",
                                    new_symbol_size, symbol_size
                                );
                                symbol_size = new_symbol_size;
                            }
                        } else {
                            println!("Received unexpected message: {}", pong_msg);
                        }
                    }
                    Err(e) => {
                        println!("Pong not received within timeout: {}", e);
                        network_stats.update(false, None);
                    }
                }
                break; // Stop sending packets after receiving pong
            }
        }

        if pong_received {
            consecutive_successes += 1;
            consecutive_failures = 0;
            if consecutive_successes >= 2 && packets_to_send > MIN_PACKETS {
                packets_to_send -= 1;
                consecutive_successes = 0;
                println!("Decreasing packets to send: {}", packets_to_send);
            }
        } else {
            consecutive_failures += 1;
            consecutive_successes = 0;
            if consecutive_failures >= 1 && packets_to_send < MAX_PACKETS {
                packets_to_send += 2;
                consecutive_failures = 0;
                println!("Increasing packets to send: {}", packets_to_send);
            }
        }

        println!(
            "Network quality: {:.2}, Current symbol size: {}, Calculated symbol size: {}",
            network_quality, symbol_size, calculated_symbol_size
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
