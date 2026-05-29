import math
import random
import socket
import struct
import time

# Network Configuration
UDP_IP = "127.0.0.1"
UDP_PORT = 8088

# Pulsar & Sampling Configuration
SAMPLING_RATE_HZ = 250000
BLOCK_SIZE = 512
PULSAR_PERIOD_TICKS = 22332  # 89.328ms
PULSAR_WIDTH_TICKS = 400  # ~1.6ms
PULSAR_AMPLITUDE = 30  # Flux above noise

# CW RFI Tone Configuration (Lands in Channel 15 / Bin 62)
RFI_FREQ_HZ = 30273.4375  # = 250000 * 62 / 512
RFI_AMPLITUDE = 40


def generate_packet(tick, seq_num, inject_rfi):
    # Word 0: Type=1 (IF Data with Stream ID), Class=0, StreamID_Indicator=1,
    #         TimestampMode=01 (UTC Integer + Sample Count), SeqNum, Size=133 words (532 bytes)
    word0 = (0x1 << 28) | (0b101 << 20) | ((seq_num & 0x0F) << 16) | 133
    stream_id = 0xDEADEBEE

    timestamp_seconds = int(time.time())
    timestamp_fraction = tick

    # 20-byte VITA-49.0 Standard Header
    header = struct.pack(
        ">IIIQ", word0, stream_id, timestamp_seconds, timestamp_fraction
    )

    # Generate 512-byte sample payload (real 8-bit time-domain samples)
    payload = bytearray(BLOCK_SIZE)
    for i in range(BLOCK_SIZE):
        t = tick + i
        phase = t % PULSAR_PERIOD_TICKS

        # 1. Base White Noise (centered at 128)
        sample_val = random.randint(108, 148)

        # 2. Inject Periodic Pulsar Pulse
        if phase < PULSAR_WIDTH_TICKS:
            sample_val += PULSAR_AMPLITUDE

        # 3. Inject Terrestrial RFI (sine-wave interference)
        if inject_rfi:
            rfi_wave = RFI_AMPLITUDE * math.sin(
                2.0 * math.pi * RFI_FREQ_HZ * t / SAMPLING_RATE_HZ
            )
            sample_val += int(rfi_wave)

        # Clamp to 0..255 byte bounds
        payload[i] = max(0, min(255, sample_val))

    return header + payload


def main():
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    print(f"PulsarSync SDR Stream Emitter started.")
    print(f"Streaming VITA-49 UDP packets to {UDP_IP}:{UDP_PORT} at 250 kHz...")

    tick = 0
    seq_num = 0
    start_time = time.time()

    # 488.28 blocks per second (512 samples at 250 kHz)
    block_period_sec = BLOCK_SIZE / SAMPLING_RATE_HZ  # = 0.002048 seconds (2.048ms)

    try:
        while True:
            elapsed = time.time() - start_time

            # Inject RFI between 4s and 10s of every 14-second cycle
            cycle_phase = elapsed % 14.0
            inject_rfi = 4.0 <= cycle_phase < 10.0

            if inject_rfi and tick % (BLOCK_SIZE * 100) == 0:
                print(
                    ">> [RFI ACTIVE] Injecting 30.2 kHz continuous wave interference..."
                )
            elif not inject_rfi and tick % (BLOCK_SIZE * 100) == 0:
                print(
                    ">> [CLEAN AIR] Streaming clean pulsar signal + background noise..."
                )

            packet = generate_packet(tick, seq_num, inject_rfi)
            sock.sendto(packet, (UDP_IP, UDP_PORT))

            tick += BLOCK_SIZE
            seq_num = (seq_num + 1) & 0x0F

            # Real-time pacing (adjust sleeping to keep exact 2.048ms block rates)
            expected_elapsed = (tick / BLOCK_SIZE) * block_period_sec
            actual_elapsed = time.time() - start_time
            sleep_dur = expected_elapsed - actual_elapsed
            if sleep_dur > 0:
                time.sleep(sleep_dur)

    except KeyboardInterrupt:
        print("\nStream Emitter stopped by user.")


if __name__ == "__main__":
    main()
