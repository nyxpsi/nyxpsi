#[cfg(test)]
mod tests {
    use rand::thread_rng;
    use rand::Rng;
    use raptorq::{Decoder, Encoder, EncodingPacket, ObjectTransmissionInformation};

    const DATA_SIZE: u64 = 1300;
    const SYMBOL_SIZE: u16 = 1000;
    const MIN_PACKETS: u32 = 5;
    const MAX_PACKETS: u32 = 20;

    #[test]
    fn test_raptorq_encoding_decoding() {
        // Create random data
        let mut data = vec![0u8; DATA_SIZE as usize];
        thread_rng().fill(&mut data[..]);

        // Create ObjectTransmissionInformation
        let oti = ObjectTransmissionInformation::with_defaults(DATA_SIZE, SYMBOL_SIZE);

        // Create encoder
        let encoder = Encoder::new(&data, oti);

        // Test with different packet counts
        for packets_to_send in MIN_PACKETS..=MAX_PACKETS {
            // Encode data
            let packets = encoder.get_encoded_packets(packets_to_send);

            // Create decoder
            let mut decoder = Decoder::new(oti);

            let mut decoded = false;
            for (i, packet) in packets.into_iter().enumerate() {
                let serialized = packet.serialize();
                let deserialized = EncodingPacket::deserialize(&serialized);

                if let Some(decoded_data) = decoder.decode(deserialized) {
                    assert_eq!(
                        decoded_data, data,
                        "Decoded data doesn't match original data"
                    );
                    println!("Successfully decoded with {} packets", i + 1);
                    decoded = true;
                    break;
                }
            }

            assert!(decoded, "Failed to decode with {} packets", packets_to_send);
        }
    }

    #[test]
    fn test_raptorq_with_packet_loss() {
        let mut data = vec![0u8; DATA_SIZE as usize];
        thread_rng().fill(&mut data[..]);

        let oti = ObjectTransmissionInformation::with_defaults(DATA_SIZE, SYMBOL_SIZE);
        let encoder = Encoder::new(&data, oti);

        let packets_to_send = MAX_PACKETS;
        let packets = encoder.get_encoded_packets(packets_to_send);

        let mut decoder = Decoder::new(oti);

        // Simulate 20% packet loss
        let mut rng = thread_rng();
        let received_packets: Vec<_> = packets
            .into_iter()
            .filter(|_| rng.gen::<f64>() > 0.2)
            .collect();

        let mut decoded = false;
        for (i, packet) in received_packets.into_iter().enumerate() {
            let serialized = packet.serialize();
            let deserialized = EncodingPacket::deserialize(&serialized);

            if let Some(decoded_data) = decoder.decode(deserialized) {
                assert_eq!(
                    decoded_data, data,
                    "Decoded data doesn't match original data"
                );
                println!(
                    "Successfully decoded with {} packets (with packet loss)",
                    i + 1
                );
                decoded = true;
                break;
            }
        }

        assert!(decoded, "Failed to decode with packet loss");
    }
}
