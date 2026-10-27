//! Sound output. A native audio callback runs on a thread of its own; WebAudio's buffer sources
//! are refilled from the main thread, and a long frame (a world loading) broke the sound up. Here
//! a thread mixes into a ring in shared memory a little ahead of an AudioWorklet that plays it, so
//! the main thread is never in the way.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use wasm_bindgen::prelude::*;

/// Frames in the ring: a power of two, more than the thread ever mixes ahead.
const FRAMES: u32 = 1 << 14;
/// How far ahead of the worklet the thread mixes: how long a new sound waits to be heard, and
/// how long the thread may be late before the worklet runs dry.
const AHEAD: Duration = Duration::from_millis(40);
/// How often the thread tops the ring up.
const PERIOD: Duration = Duration::from_millis(4);

#[wasm_bindgen(inline_js = r#"
// Plays the frames that src/audio.rs mixes into its ring, and counts the quanta it finds short.
const OUTPUT = `
class HorizonOutput extends AudioWorkletProcessor {
    constructor({ processorOptions: { buffer, counters, samples, frames, channels } }) {
        super();
        this.counters = new Uint32Array(buffer, counters, 3);
        this.samples = new Float32Array(buffer, samples, frames * channels);
        this.mask = frames - 1;
        this.channels = channels;
    }
    process(inputs, [output]) {
        const length = output[0].length;
        const played = Atomics.load(this.counters, 0);
        const ready = Math.min(length, (Atomics.load(this.counters, 1) - played) >>> 0);
        for (let c = 0; c < output.length; c++) {
            const out = output[c];
            const channel = Math.min(c, this.channels - 1);
            for (let i = 0; i < ready; i++) {
                out[i] = this.samples[((played + i) & this.mask) * this.channels + channel];
            }
            out.fill(0, ready);
        }
        Atomics.store(this.counters, 0, (played + ready) >>> 0);
        if (ready < length) Atomics.add(this.counters, 2, 1);
        return true;
    }
}
registerProcessor('horizon-output', HorizonOutput);
`;
// A browser starts sound only after a gesture: a click or key on the page resumes the output.
export function audio_open(memory, counters, samples, frames, channels) {
    const context = new AudioContext({ latencyHint: 'interactive' });
    context.horizonResume = () => context.state === 'suspended' && context.resume();
    for (const type of ['pointerdown', 'keydown']) addEventListener(type, context.horizonResume, true);
    const url = URL.createObjectURL(new Blob([OUTPUT], { type: 'text/javascript' }));
    context.audioWorklet
        .addModule(url)
        .then(() => {
            const options = { buffer: memory.buffer, counters, samples, frames, channels };
            const node = new AudioWorkletNode(context, 'horizon-output', {
                numberOfInputs: 0,
                outputChannelCount: [channels],
                processorOptions: options,
            });
            node.connect(context.destination);
        })
        .catch((error) => context.state !== 'closed' && console.error('horizon-web: sound output', error))
        .finally(() => URL.revokeObjectURL(url));
    return context;
}
export function audio_rate(context) {
    return context.sampleRate;
}
export function audio_close(context) {
    for (const type of ['pointerdown', 'keydown']) removeEventListener(type, context.horizonResume, true);
    context.close();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    fn audio_open(
        memory: &JsValue,
        counters: u32,
        samples: u32,
        frames: u32,
        channels: u32,
    ) -> Result<JsValue, JsValue>;
    fn audio_rate(context: &JsValue) -> f64;
    fn audio_close(context: &JsValue);
}

struct Ring {
    /// Frames the worklet played, frames the thread mixed (both wrap), and the worklet's quanta
    /// that ran dry.
    counters: [AtomicU32; 3],
    stop: AtomicBool,
    /// Interleaved f32 samples, as bits.
    samples: Box<[AtomicU32]>,
}

/// The page's sound output (the client's `audio` feature): stereo, at the output's own rate.
pub struct AudioOutput {
    context: JsValue,
    ring: &'static Ring,
    rate: u32,
}

impl AudioOutput {
    pub const CHANNELS: usize = 2;

    /// Opens the output; `mix(frames, channels, rate)` fills interleaved frames on a thread of its
    /// own, as a native output callback.
    pub fn open(
        mut mix: impl FnMut(&mut [f32], usize, u32) + Send + 'static,
    ) -> Result<Self, String> {
        let channels = Self::CHANNELS;
        // The worklet keeps reading the ring until the context closes: it is never freed.
        let ring: &'static Ring = Box::leak(Box::new(Ring {
            counters: Default::default(),
            stop: AtomicBool::new(false),
            samples: (0..FRAMES as usize * channels)
                .map(|_| AtomicU32::new(0))
                .collect(),
        }));
        let context = audio_open(
            &wasm_bindgen::memory(),
            ring.counters.as_ptr() as u32,
            ring.samples.as_ptr() as u32,
            FRAMES,
            channels as u32,
        )
        .map_err(|e| format!("{e:?}"))?;
        let rate = audio_rate(&context) as u32;
        let ahead = (AHEAD.as_secs_f64() * f64::from(rate)) as u32;
        let thread = std::thread::Builder::new()
            .name("Sound output".into())
            .spawn(move || {
                let [played, mixed, dry] = &ring.counters;
                let mut frames = Vec::new();
                let (mut reported, mut last_report) = (0, Instant::now());
                while !ring.stop.load(Ordering::Relaxed) {
                    let start = mixed.load(Ordering::Relaxed);
                    let wanted =
                        ahead.saturating_sub(start.wrapping_sub(played.load(Ordering::Acquire)));
                    if wanted > 0 {
                        frames.resize(wanted as usize * channels, 0.0);
                        mix(&mut frames, channels, rate);
                        for (i, frame) in frames.chunks_exact(channels).enumerate() {
                            let at =
                                (start.wrapping_add(i as u32) & (FRAMES - 1)) as usize * channels;
                            for (slot, sample) in ring.samples[at..at + channels].iter().zip(frame)
                            {
                                slot.store(sample.to_bits(), Ordering::Relaxed);
                            }
                        }
                        mixed.store(start.wrapping_add(wanted), Ordering::Release);
                    }
                    let short = dry.load(Ordering::Relaxed);
                    if short != reported && last_report.elapsed() >= Duration::from_secs(1) {
                        eprintln!(
                            "horizon-web: the sound output ran dry {} times",
                            short.wrapping_sub(reported)
                        );
                        (reported, last_report) = (short, Instant::now());
                    }
                    std::thread::sleep(PERIOD);
                }
            });
        if let Err(e) = thread {
            audio_close(&context);
            return Err(e.to_string());
        }
        Ok(Self {
            context,
            ring,
            rate,
        })
    }

    /// Frames per second.
    pub fn rate(&self) -> u32 {
        self.rate
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        self.ring.stop.store(true, Ordering::Relaxed);
        audio_close(&self.context);
    }
}
