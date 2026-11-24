// RustDesk sends one raw Opus packet per audio frame.
export class RemoteAudio {
  private context = new AudioContext();
  private decoder: AudioDecoder | null = null;
  private nextStart = 0;

  constructor(private onError: () => void) {}

  resume() {
    return this.context.resume();
  }

  configure(sampleRate: number, channels: number) {
    try {
      if (this.decoder && this.decoder.state !== 'closed') this.decoder.close();
      this.decoder = new AudioDecoder({
        output: frame => {
          try {
            if (this.context.state !== 'running') return;
            const now = this.context.currentTime;
            if (this.nextStart > now + 0.3) return;
            const buffer = this.context.createBuffer(frame.numberOfChannels, frame.numberOfFrames, frame.sampleRate);
            for (let channel = 0; channel < buffer.numberOfChannels; channel++) {
              frame.copyTo(buffer.getChannelData(channel), { planeIndex: channel, format: 'f32-planar' });
            }
            const source = this.context.createBufferSource();
            source.buffer = buffer;
            source.connect(this.context.destination);
            const start = Math.max(now + 0.01, this.nextStart);
            source.start(start);
            this.nextStart = start + buffer.duration;
          } catch {
            this.onError();
          } finally {
            frame.close();
          }
        },
        error: this.onError,
      });
      this.decoder.configure({ codec: 'opus', sampleRate, numberOfChannels: channels });
    } catch {
      this.onError();
    }
  }

  push(packet: string) {
    if (this.decoder?.state !== 'configured' || this.decoder.decodeQueueSize > 12) return;
    try {
      const data = Uint8Array.from(atob(packet), c => c.charCodeAt(0));
      this.decoder.decode(new EncodedAudioChunk({ type: 'key', timestamp: Math.round(performance.now() * 1000), data }));
    } catch {
      this.onError();
    }
  }

  close() {
    if (this.decoder && this.decoder.state !== 'closed') this.decoder.close();
    void this.context.close().catch(() => {});
  }
}
