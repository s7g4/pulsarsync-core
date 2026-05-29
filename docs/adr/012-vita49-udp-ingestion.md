# ADR-012: VITA-49 Radio Transport Packet Ingestion over UDP

## Status
Approved

## Context
In modern Software-Defined Radio (SDR) systems, digitized RF sample streams are transported over high-speed networks from the antenna/digitizer node to the DSP processor. We need a standardized, structured transport protocol to ingest raw samples into our gateway pipeline, with support for sample synchronization, stream identification, and packet loss detection.

## Decision
We implement a **VITA-49.0 (VITA Radio Transport, VRT)** packet receiver over UDP.
* The VITA-49 packet structure includes:
  - 32-bit Header containing Packet Type, Sequence Number, and Packet Size.
  - 32-bit Stream ID to differentiate RF signals.
  - 32-bit Integer Timestamp and 64-bit Fractional Timestamp for sample-accurate time-tagging.
  - 512-byte raw real 8-bit sample payload.
* The receiver binds to a UDP socket (port 8088) on the host gateway daemon when `host-testing` is enabled, parses the header, verifies packet continuity using the 4-bit sequence counter, and pipes the payload into the SPSC ring buffer.

## Rationale
1. **Industry Standardization**:
   * VITA-49 is the dominant standard in aerospace, defense, and signal intelligence for SDR interfaces (used by USRPs, signal analyzers, and receivers).
2. **Sequence Gap Detection**:
   * UDP is an unreliable protocol that can drop packets under high network loads.
   * VITA-49's 4-bit sequence counter (modulo 16) allows the ingestion layer to detect missing packets and update the `METRIC_BLOCKS_DROPPED` metric in real-time.
3. **Zero-Copy Alignment**:
   * By matching the VITA-49 payload size (512 bytes) to our internal pipeline block size (`BLOCK_SIZE = 512`), we can copy samples directly from the network buffer into the aligned `SampleBlock` memory pool slot, avoiding buffer fragmentation.

## Consequences
* The host gateway daemon can process live streaming data from any compatible VITA-49 SDR sender or simulation script.
* Slower UDP performance or network congestion will trigger telemetry alerts in the dashboard.
