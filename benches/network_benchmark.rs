use criterion::BenchmarkId;
use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;
use raptorq::{Decoder, Encoder, ObjectTransmissionInformation};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

const DATA_SIZE: usize = 1_000_000; // 1 MB
const SYMBOL_SIZE: u16 = 1000;
const LATENCY_MS: u64 = 1; // 1ms latency
const BENCHMARK_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct BenchmarkResult {
    duration: Duration,
    packets_sent: usize,
    packets_received: usize,
    transfer_success: bool,
}

fn simulate_network_latency() {
    thread::sleep(Duration::from_millis(LATENCY_MS));
}

fn simulate_packet_loss(loss_rate: f64) -> bool {
    rand::thread_rng().gen::<f64>() > loss_rate
}

fn generate_random_data() -> Vec<u8> {
    let mut data = vec![0u8; DATA_SIZE];
    rand::thread_rng().fill(&mut data[..]);
    data
}

fn benchmark_raptorq(loss_rate: f64) -> BenchmarkResult {
    let data = generate_random_data();
    let start = Instant::now();

    let oti = ObjectTransmissionInformation::with_defaults(DATA_SIZE as u64, SYMBOL_SIZE);
    let encoder = Encoder::new(&data, oti);
    let mut decoder = Decoder::new(oti);

    let mut packets_sent = 0;
    let mut packets_received = 0;
    let mut decoded_data: Option<Vec<u8>> = None;

    println!("Starting RaptorQ benchmark with loss rate: {}", loss_rate);

    // Calculate the number of packets needed (with some redundancy)
    let packets_needed = (DATA_SIZE / SYMBOL_SIZE as usize) as u32;
    let total_packets = (packets_needed as f64 * (1.0 + loss_rate) * 1.1) as u32; // Add 10% extra

    for packet in encoder.get_encoded_packets(total_packets) {
        packets_sent += 1;

        if simulate_packet_loss(loss_rate) {
            simulate_network_latency();
            packets_received += 1;

            if let Some(data) = decoder.decode(packet) {
                decoded_data = Some(data);
                println!("Successfully decoded data after {} packets", packets_sent);
                break;
            }
        }

        if packets_sent % 1000 == 0 || packets_sent == total_packets as usize {
            println!(
                "Sent {} packets, received {}",
                packets_sent, packets_received
            );
        }

        if start.elapsed() >= BENCHMARK_TIMEOUT {
            println!("Benchmark timed out");
            break;
        }
    }

    let duration = start.elapsed();
    let success = decoded_data.as_ref().map_or(false, |d| d == &data);
    println!(
        "RaptorQ benchmark completed in {:?}, success: {}",
        duration, success
    );

    BenchmarkResult {
        duration,
        packets_sent,
        packets_received,
        transfer_success: success,
    }
}

fn benchmark_tcp(loss_rate: f64) -> BenchmarkResult {
    let data = generate_random_data();
    let start = Instant::now();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let data_clone = data.clone();
    let sender = thread::spawn(move || {
        let mut stream = TcpStream::connect(addr).unwrap();
        let mut packets_sent = 0;
        for chunk in data_clone.chunks(1024) {
            packets_sent += 1;
            if simulate_packet_loss(loss_rate) {
                simulate_network_latency();
                stream.write_all(chunk).unwrap();
            }
        }
        packets_sent
    });

    let receiver = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut received_data = Vec::new();
        let mut packets_received = 0;
        let mut buf = [0u8; 1024];
        while let Ok(n) = stream.read(&mut buf) {
            if n == 0 {
                break;
            }
            packets_received += 1;
            received_data.extend_from_slice(&buf[..n]);
        }
        (received_data, packets_received)
    });

    let packets_sent = sender.join().unwrap();
    let (received_data, packets_received) = receiver.join().unwrap();
    let duration = start.elapsed();

    BenchmarkResult {
        duration,
        packets_sent,
        packets_received,
        transfer_success: received_data == data,
    }
}

fn benchmark_udp(loss_rate: f64) -> BenchmarkResult {
    let data = generate_random_data();
    let start = Instant::now();

    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    let recv_addr = receiver.local_addr().unwrap();

    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();

    let data_clone = data.clone();
    let sender_thread = thread::spawn(move || {
        let mut packets_sent = 0;
        for chunk in data_clone.chunks(1024) {
            packets_sent += 1;
            if simulate_packet_loss(loss_rate) {
                simulate_network_latency();
                sender.send_to(chunk, recv_addr).unwrap();
            }
        }
        packets_sent
    });

    let receiver_thread = thread::spawn(move || {
        let mut buf = [0u8; 1024];
        let mut received_data = Vec::new();
        let mut packets_received = 0;
        let timeout = Instant::now() + Duration::from_secs(5); // 5 second overall timeout

        while received_data.len() < DATA_SIZE && Instant::now() < timeout {
            match receiver.recv_from(&mut buf) {
                Ok((size, _)) => {
                    packets_received += 1;
                    received_data.extend_from_slice(&buf[..size]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Timeout occurred, continue the loop
                    continue;
                }
                Err(_) => break,
            }
        }
        (received_data, packets_received)
    });

    let packets_sent = sender_thread.join().unwrap();
    let (received_data, packets_received) = receiver_thread.join().unwrap();
    let duration = start.elapsed();

    BenchmarkResult {
        duration,
        packets_sent,
        packets_received,
        transfer_success: received_data == data,
    }
}

fn run_benchmarks(c: &mut Criterion) {
    let loss_rates = [0.0, 0.1, 0.5, 0.9];
    let protocols = ["RaptorQ", "TCP", "UDP"];

    let mut group = c.benchmark_group("Network Protocols");
    group.sample_size(10); // 10 samples
    group.measurement_time(Duration::from_secs(60)); // Increase to 60 seconds for more stable results
    group.warm_up_time(Duration::from_secs(5)); // Add warm-up time

    for &loss_rate in &loss_rates {
        for &protocol in &protocols {
            group.bench_with_input(
                BenchmarkId::new(protocol, format!("{:.0}% loss", loss_rate * 100.0)),
                &loss_rate,
                |b, &loss_rate| {
                    b.iter_custom(|iters| {
                        let mut total_duration = Duration::ZERO;
                        let mut total_packets_sent = 0u64;
                        let mut total_packets_received = 0u64;
                        let mut successes = 0u64;

                        for _ in 0..iters {
                            let result = match protocol {
                                "RaptorQ" => benchmark_raptorq(loss_rate),
                                "TCP" => benchmark_tcp(loss_rate),
                                "UDP" => benchmark_udp(loss_rate),
                                _ => unreachable!(),
                            };
                            total_duration += result.duration;
                            total_packets_sent += result.packets_sent as u64;
                            total_packets_received += result.packets_received as u64;
                            if result.transfer_success {
                                successes += 1;
                            }
                        }

                        println!(
                            "Protocol: {}, Loss Rate: {:.1}, Iterations: {}",
                            protocol, loss_rate, iters
                        );
                        println!("Average Duration: {:?}", total_duration / iters as u32);
                        println!("Average Packets Sent: {}", total_packets_sent / iters);
                        println!(
                            "Average Packets Received: {}",
                            total_packets_received / iters
                        );
                        println!(
                            "Success Rate: {:.2}%",
                            (successes as f64 / iters as f64) * 100.0
                        );

                        total_duration
                    })
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, run_benchmarks);
criterion_main!(benches);
