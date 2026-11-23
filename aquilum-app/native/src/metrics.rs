//! Frame measurements. Written to the `--metrics` JSON file, which `scripts/measure.ps1` reads.

use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Metrics {
    path: Option<PathBuf>,
    threads: u16,
    scale_factor: f64,
    window_created_ms: f64,
    first_frame_ms: f64,
    pub frames: u64,
    /// Masonry animation ticks; a frame results from them only when there are visible changes.
    pub anim_ticks: u64,
    /// Frames after the first: rasterization and the full frame (including the buffer copy and present), ms.
    raster: Vec<f64>,
    total: Vec<f64>,
    #[cfg(not(feature = "installed"))]
    live: Option<Live>,
}

#[cfg(not(feature = "installed"))]
#[derive(Default)]
struct Live {
    since: Option<Instant>,
    frames: u32,
    sums: [Duration; 4],
    worst: Duration,
    summary: String,
}

#[cfg(not(feature = "installed"))]
impl Live {
    fn frame(&mut self, parts: [Duration; 4]) -> bool {
        let since = *self.since.get_or_insert_with(Instant::now);
        self.frames += 1;
        for (sum, part) in self.sums.iter_mut().zip(parts) {
            *sum += part;
        }
        self.worst = self.worst.max(parts[3]);
        let elapsed = since.elapsed();
        if elapsed < Duration::from_secs(1) {
            return false;
        }
        let avg = |d: Duration| ms(d) / f64::from(self.frames);
        self.summary = format!(
            "{:.0} fps · frame {:.1} ms, worst {:.1} (scene {:.1}, raster {:.1}, output {:.1})",
            f64::from(self.frames) / elapsed.as_secs_f64(),
            avg(self.sums[3]),
            ms(self.worst),
            avg(self.sums[0]),
            avg(self.sums[1]),
            avg(self.sums[2]),
        );
        *self = Live { summary: std::mem::take(&mut self.summary), ..Live::default() };
        true
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

impl Metrics {
    pub fn new(threads: u16, path: Option<PathBuf>) -> Self {
        Metrics { path, threads, ..Default::default() }
    }

    pub fn window_created(&mut self, started: Instant, scale_factor: f64) {
        self.window_created_ms = ms(started.elapsed());
        self.scale_factor = scale_factor;
    }

    pub fn frame(&mut self, started: Instant, parts: [Duration; 4]) -> bool {
        let [_, raster, _, total] = parts;
        self.frames += 1;
        if self.frames == 1 {
            self.first_frame_ms = ms(started.elapsed());
        } else if self.path.is_some() {
            self.raster.push(ms(raster));
            self.total.push(ms(total));
        }
        #[cfg(not(feature = "installed"))]
        if let Some(live) = &mut self.live {
            return live.frame(parts);
        }
        false
    }

    #[cfg(not(feature = "installed"))]
    pub fn toggle_live(&mut self) {
        self.live = if self.live.is_some() { None } else { Some(Live::default()) };
    }

    pub fn summary(&self) -> Option<&str> {
        #[cfg(not(feature = "installed"))]
        if let Some(live) = &self.live {
            return Some(&live.summary);
        }
        None
    }

    pub fn write(&self) {
        let Some(path) = &self.path else { return };
        let json = format!(
            concat!(
                "{{\"threads\":{},\"scaleFactor\":{},\"windowCreatedMs\":{:.1},\"firstFrameMs\":{:.1},",
                "\"frames\":{},\"animTicks\":{},\"rasterAvgMs\":{:.2},\"rasterP95Ms\":{:.2},",
                "\"frameAvgMs\":{:.2},\"frameP95Ms\":{:.2}}}"
            ),
            self.threads,
            self.scale_factor,
            self.window_created_ms,
            self.first_frame_ms,
            self.frames,
            self.anim_ticks,
            crate::stats::mean(&self.raster),
            crate::stats::percentiles(&self.raster, [0.95])[0],
            crate::stats::mean(&self.total),
            crate::stats::percentiles(&self.total, [0.95])[0],
        );
        if let Err(e) = std::fs::write(path, &json) {
            eprintln!("failed to write measurements to {}: {e}", path.display());
        }
    }
}
