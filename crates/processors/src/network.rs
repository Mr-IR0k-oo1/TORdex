//! Network processor — analyzes pcap captures and packet data.

use std::collections::HashMap;

use tordex_core::processor::{ProcessedObservation, Processor, ProcessorError};

pub struct NetworkProcessor;

impl NetworkProcessor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn parse_pcap_global_header(&self, data: &[u8]) -> Option<PcapHeader> {
        if data.len() < 24 {
            return None;
        }
        // Check magic number
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let (version_major, version_minor, thiszone, sigfigs, snaplen, network) = match magic {
            0xA1B2C3D4 | 0xD4C3B2A1 => {
                // Little-endian or big-endian pcap
                let major = u16::from_le_bytes([data[4], data[5]]);
                let minor = u16::from_le_bytes([data[6], data[7]]);
                let zone = i32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                let sigs = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
                let snap = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
                let net = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
                (major, minor, zone, sigs, snap, net)
            }
            _ => return None,
        };

        let link_type = match network {
            1 => "Ethernet",
            101 => "Raw IP",
            0 => "Null/Loopback",
            108 => "Linux SLL",
            113 => "Linux SLL2",
            _ => "Unknown",
        };

        Some(PcapHeader {
            version_major,
            version_minor,
            thiszone,
            sigfigs,
            snaplen,
            network,
            link_type: link_type.to_string(),
        })
    }

    fn count_packets(&self, data: &[u8]) -> usize {
        if data.len() < 24 {
            return 0;
        }
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let swap = magic == 0xD4C3B2A1;
        let header_len = 24;
        let mut pos = header_len;
        let mut count = 0;

        while pos + 16 <= data.len() {
            // Packet header: 4 bytes ts_sec, 4 bytes ts_usec, 4 bytes incl_len, 4 bytes orig_len
            let incl_len = if swap {
                u32::from_be_bytes([data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11]])
            } else {
                u32::from_le_bytes([data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11]])
            };
            let pkt_len = 16 + incl_len as usize;
            if pkt_len == 16 {
                break;
            }
            pos += pkt_len;
            count += 1;
            if pos > data.len() {
                break;
            }
        }

        count
    }
}

impl Default for NetworkProcessor {
    fn default() -> Self {
        Self::new()
    }
}

struct PcapHeader {
    #[allow(dead_code)]
    version_major: u16,
    #[allow(dead_code)]
    version_minor: u16,
    #[allow(dead_code)]
    thiszone: i32,
    #[allow(dead_code)]
    sigfigs: u32,
    #[allow(dead_code)]
    snaplen: u32,
    #[allow(dead_code)]
    network: u32,
    link_type: String,
}

impl Processor for NetworkProcessor {
    fn name(&self) -> &str {
        "network"
    }

    fn description(&self) -> &str {
        "Analyzes network captures (pcap) and extracts packet metadata"
    }

    fn content_types(&self) -> Vec<&str> {
        vec![
            "application/vnd.tcpdump.pcap",
            "application/x-pcapng",
            "application/x-pcap",
            "application/x-pcapng",
        ]
    }

    fn process(
        &self,
        id: &str,
        data: &[u8],
        _content_type: Option<&str>,
        _metadata: HashMap<String, String>,
    ) -> Result<Vec<ProcessedObservation>, ProcessorError> {
        let mut results = Vec::new();

        if let Some(header) = self.parse_pcap_global_header(data) {
            results.push(
                ProcessedObservation::new(
                    format!("{id}_link_type"),
                    "network.link_type",
                    header.link_type.as_bytes().to_vec(),
                    "text/plain",
                )
                .with_metadata("source_observation", id),
            );

            let packet_count = self.count_packets(data);
            results.push(
                ProcessedObservation::new(
                    format!("{id}_packets"),
                    "network.packet_count",
                    packet_count.to_string().into_bytes(),
                    "text/plain",
                )
                .with_metadata("count", &packet_count.to_string())
                .with_metadata("source_observation", id),
            );
        }

        results.push(
            ProcessedObservation::new(
                format!("{id}_size"),
                "network.metadata",
                data.len().to_string().into_bytes(),
                "text/plain",
            )
            .with_metadata("metric", "byte_size")
            .with_metadata("value", &data.len().to_string())
            .with_metadata("source_observation", id),
        );

        if results.is_empty() {
            return Err(ProcessorError::ProcessingFailed(
                "no pcap header detected".into(),
            ));
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pcap() -> Vec<u8> {
        let mut data = Vec::new();
        // Global header: magic, major, minor, tz, sigfigs, snaplen, network
        data.extend_from_slice(&0xA1B2C3D4u32.to_le_bytes()); // magic
        data.extend_from_slice(&2u16.to_le_bytes()); // major
        data.extend_from_slice(&4u16.to_le_bytes()); // minor
        data.extend_from_slice(&0i32.to_le_bytes()); // thiszone
        data.extend_from_slice(&0u32.to_le_bytes()); // sigfigs
        data.extend_from_slice(&65535u32.to_le_bytes()); // snaplen
        data.extend_from_slice(&1u32.to_le_bytes()); // network (Ethernet)
        // One packet: ts_sec, ts_usec, incl_len, orig_len + data
        data.extend_from_slice(&1000000u32.to_le_bytes()); // ts_sec
        data.extend_from_slice(&0u32.to_le_bytes()); // ts_usec
        data.extend_from_slice(&4u32.to_le_bytes()); // incl_len
        data.extend_from_slice(&4u32.to_le_bytes()); // orig_len
        data.extend_from_slice(b"\x00\x01\x02\x03"); // packet data
        data
    }

    #[test]
    fn detect_pcap_format() {
        let proc = NetworkProcessor::new();
        let pcap = make_pcap();
        let results = proc.process("n1", &pcap, Some("application/vnd.tcpdump.pcap"), HashMap::new()).unwrap();
        let link_types: Vec<_> = results.iter().filter(|o| o.kind == "network.link_type").collect();
        assert!(!link_types.is_empty());
        assert_eq!(std::str::from_utf8(&link_types[0].data).unwrap(), "Ethernet");
    }

    #[test]
    fn count_packets() {
        let proc = NetworkProcessor::new();
        let pcap = make_pcap();
        let results = proc.process("n2", &pcap, Some("application/vnd.tcpdump.pcap"), HashMap::new()).unwrap();
        let counts: Vec<_> = results.iter().filter(|o| o.kind == "network.packet_count").collect();
        assert!(!counts.is_empty());
        assert_eq!(std::str::from_utf8(&counts[0].data).unwrap(), "1");
    }

    #[test]
    fn unknown_format_errors() {
        let proc = NetworkProcessor::new();
        let data = b"\x00\x01\x02\x03";
        let result = proc.process("n3", data, Some("application/octet-stream"), HashMap::new());
        assert!(result.is_err());
    }
}
