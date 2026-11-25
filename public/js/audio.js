// Voxelheim — tiny synthesized sound kit (WebAudio, no assets).

export class Sfx {
  constructor() {
    this.ctx = null;
    this.master = null;
    this.muted = false;
  }

  ensure() {
    if (this.ctx) { if (this.ctx.state === 'suspended') this.ctx.resume(); return true; }
    try {
      const AC = window.AudioContext || window.webkitAudioContext;
      if (!AC) return false;
      this.ctx = new AC();
      this.master = this.ctx.createGain();
      this.master.gain.value = 0.3;
      this.master.connect(this.ctx.destination);
      return true;
    } catch { return false; }
  }

  toggleMute() {
    this.muted = !this.muted;
    if (this.master) this.master.gain.value = this.muted ? 0 : 0.3;
    return this.muted;
  }

  tone({ freq = 440, end = null, dur = 0.1, type = 'square', vol = 0.4, delay = 0 }) {
    if (!this.ctx || this.muted) return;
    const t0 = this.ctx.currentTime + delay;
    const osc = this.ctx.createOscillator();
    const g = this.ctx.createGain();
    osc.type = type;
    osc.frequency.setValueAtTime(freq, t0);
    if (end !== null) osc.frequency.exponentialRampToValueAtTime(Math.max(20, end), t0 + dur);
    g.gain.setValueAtTime(vol, t0);
    g.gain.exponentialRampToValueAtTime(0.001, t0 + dur);
    osc.connect(g); g.connect(this.master);
    osc.start(t0); osc.stop(t0 + dur + 0.02);
  }

  noise({ dur = 0.12, freq = 900, vol = 0.4, type = 'lowpass' }) {
    if (!this.ctx || this.muted) return;
    const t0 = this.ctx.currentTime;
    const len = Math.max(1, Math.floor(this.ctx.sampleRate * dur));
    const buf = this.ctx.createBuffer(1, len, this.ctx.sampleRate);
    const d = buf.getChannelData(0);
    for (let i = 0; i < len; i++) d[i] = (Math.random() * 2 - 1) * (1 - i / len);
    const src = this.ctx.createBufferSource();
    src.buffer = buf;
    const f = this.ctx.createBiquadFilter();
    f.type = type; f.frequency.value = freq;
    const g = this.ctx.createGain();
    g.gain.value = vol;
    src.connect(f); f.connect(g); g.connect(this.master);
    src.start(t0);
  }

  place() { this.tone({ freq: 520, end: 240, dur: 0.07, type: 'triangle', vol: 0.5 }); this.noise({ dur: 0.04, freq: 1400, vol: 0.12 }); }
  break_() { this.noise({ dur: 0.13, freq: 800, vol: 0.5 }); this.tone({ freq: 180, end: 90, dur: 0.08, type: 'triangle', vol: 0.25 }); }
  dig() { this.noise({ dur: 0.05, freq: 600, vol: 0.18 }); }
  hit() { this.tone({ freq: 220, end: 90, dur: 0.1, type: 'square', vol: 0.4 }); this.noise({ dur: 0.05, freq: 500, vol: 0.2 }); }
  swing() { this.noise({ dur: 0.06, freq: 2400, vol: 0.08, type: 'highpass' }); }
  hurt() { this.tone({ freq: 320, end: 110, dur: 0.22, type: 'sawtooth', vol: 0.4 }); }
  mobHurt() { this.tone({ freq: 420, end: 160, dur: 0.12, type: 'square', vol: 0.3 }); }
  die() { this.tone({ freq: 300, end: 50, dur: 0.3, type: 'square', vol: 0.35 }); }
  hop() { this.tone({ freq: 190, end: 260, dur: 0.06, type: 'sine', vol: 0.14 }); }
  splash() { this.noise({ dur: 0.22, freq: 700, vol: 0.3 }); }
  click() { this.tone({ freq: 900, end: 700, dur: 0.04, type: 'sine', vol: 0.2 }); }
  chime() { this.tone({ freq: 660, dur: 0.12, type: 'sine', vol: 0.2 }); this.tone({ freq: 880, dur: 0.16, type: 'sine', vol: 0.16, delay: 0.1 }); }
}
