// server.rs
use raptorq::{Decoder, EncodingPacket, ObjectTransmissionInformation};
use std::{error::Error, net::SocketAddr};
use tokio::time::Instant;
use udplite::UdpLiteSocket;

const DATA_SIZE: u64 = 1300;
const MAX_SYMBOL_SIZE: u16 = 2000;
const MIN_SYMBOL_SIZE: u16 = 500;
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
    let addr: SocketAddr = "127.0.0.1:55555".parse()?;
    let socket = UdpLiteSocket::bind(addr)?;
    socket.set_recv_checksum_coverage_filter(Some(8))?;

    println!("Server listening on: {}", addr);

    let mut network_stats = NetworkStats::new();
    let mut current_symbol_size = MIN_SYMBOL_SIZE; // Start with the minimum symbol size
    let mut packets_received = 0;
    let mut current_decoder: Option<Decoder> = None;

    loop {
        let mut buf = [0u8; 2000];
        let start_time = Instant::now();
        match socket.recv_from(&mut buf) {
            Ok((size, src_addr)) => {
                let latency = start_time.elapsed().as_millis();
                network_stats.update(true, Some(latency));
                packets_received += 1;
                println!(
                    "Received packet {} from {} with size {}",
                    packets_received, src_addr, size
                );

                let packet = EncodingPacket::deserialize(&buf[..size]);
                let packet_symbol_size = size as u16; // Use the received packet size as the symbol size

                if current_decoder.is_none() || packet_symbol_size != current_symbol_size {
                    println!(
                        "Creating new decoder with symbol size: {}",
                        packet_symbol_size
                    );
                    let oti =
                        ObjectTransmissionInformation::with_defaults(DATA_SIZE, packet_symbol_size);
                    current_decoder = Some(Decoder::new(oti));
                    current_symbol_size = packet_symbol_size;
                }

                if let Some(ref mut decoder) = current_decoder {
                    println!(
                        "Attempting to decode packet with symbol size: {}",
                        current_symbol_size
                    );
                    match decoder.decode(packet) {
                        Some(decoded_data) => {
                            println!(
                                "Decoded {} bytes from {} after {} packets",
                                decoded_data.len(),
                                src_addr,
                                packets_received
                            );
                            let network_quality = network_stats.get_network_quality();
                            let next_symbol_size = calculate_symbol_size(network_quality);

                            let pong_msg = format!("Meow:{}", next_symbol_size);
                            if let Err(e) = socket.send_to(pong_msg.as_bytes(), src_addr) {
                                println!("Failed to send Pong to {}: {}", src_addr, e);
                            } else {
                                println!(
                                    "Pong sent successfully to {} with next symbol size {}",
                                    src_addr, next_symbol_size
                                );
                            }

                            if next_symbol_size != current_symbol_size {
                                println!("Symbol size will be adjusted from {} to {} based on network quality {:.2}", 
                                         current_symbol_size, next_symbol_size, network_quality);
                                current_symbol_size = next_symbol_size;
                            }

                            current_decoder = None;
                            packets_received = 0;
                            println!("Ready for next message from {}", src_addr);
                        }
                        None => {
                            println!("Packet added to decoder, but message not yet complete. Continuing to receive more packets.");
                        }
                    }
                } else {
                    println!("Error: Decoder not initialized");
                }
            }
            Err(e) => {
                println!("Error receiving UDP-Lite packet: {}", e);
                network_stats.update(false, None);
            }
        }
    }
}
