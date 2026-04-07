use crate::now_playing_update_message::NowPlayingInfo;
use anyhow::anyhow;
use block2::RcBlock;
use objc2::__framework_prelude::Retained;
use objc2_foundation::{ns_string, NSString, NSTimer};
use std::cell::OnceCell;
use std::cmp::Ordering;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use objc2::rc::Weak;
use objc2_app_kit::NSTextField;
use crate::lyrics_line::LyricsLine;

const LYRICS_UPDATE_TIME_INTERVAL: f64 = 0.1f64;
const LYRICS_TIMING_OFFSET: f64 = 0.25f64;

#[derive(Debug)]
pub struct LyricsSyncer {
    timer: OnceCell<Retained<NSTimer>>,
    lyrics_line_text_view: Weak<NSTextField>,
    pub now_playing_info: Arc<Mutex<Option<NowPlayingInfo>>>,
    last_now_playing_info_update_timestamp: Mutex<Option<Instant>>,
    pub lyrics: Arc<Mutex<Option<Vec<LyricsLine>>>>
}

impl LyricsSyncer {
    pub fn new(lyrics_line_text_view: &Retained<NSTextField>) -> anyhow::Result<Arc<Self>> {
        unsafe {
            let self_arc = Arc::new(Self {
                timer: OnceCell::new(),
                lyrics_line_text_view: Weak::from_retained(lyrics_line_text_view),
                now_playing_info: Arc::new(Mutex::new(None)),
                last_now_playing_info_update_timestamp: Mutex::new(None),
                lyrics: Arc::new(Mutex::new(None))
            });

            let self_arc_clone = self_arc.clone();

            let timer = NSTimer::scheduledTimerWithTimeInterval_repeats_block(
                LYRICS_UPDATE_TIME_INTERVAL,
                true,
                &RcBlock::new(move |_timer: NonNull<NSTimer>| {
                    self_arc_clone.run();
                })
            );

            self_arc.timer.set(timer).map_err(|_| anyhow!("Failed to set timer"))?;

            Ok(self_arc)
        }
    }

    fn run(&self) {
        let Ok(now_playing_info_guard) = self.now_playing_info.lock() else { return; };
        let Some(now_playing_info) = (*now_playing_info_guard).clone() else { return };

        if now_playing_info.playback_rate == 0f64 {
            return;
        }

        let Ok(last_now_playing_info_update_timestamp_guard) = self.last_now_playing_info_update_timestamp.lock() else { return; };
        let Ok(lyrics_guard) = self.lyrics.lock() else { return; };
        let Some(last_now_playing_info_update_timestamp) = (*last_now_playing_info_update_timestamp_guard).clone() else { return };
        let Some(lyrics) = (*lyrics_guard).clone() else { return };

        let Some(lyrics_line_text_view) = self.lyrics_line_text_view.load() else { return; };

        let seconds_since_last_update = Instant::now().duration_since(last_now_playing_info_update_timestamp).as_secs_f64();

        let current_position_seconds = now_playing_info.current_position_seconds + (seconds_since_last_update * now_playing_info.playback_rate) + LYRICS_TIMING_OFFSET;

        if let Some(current_lyrics_line) = lyrics.iter()
            .filter(|line| line.seconds <= current_position_seconds)
            .max_by(|a, b| {
                if a.seconds > b.seconds {
                    Ordering::Greater
                } else if b.seconds > a.seconds {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            }) /* .or(lyrics.first()) */
        {
            lyrics_line_text_view.setStringValue(&NSString::from_str(current_lyrics_line.content.trim()));
        } else {
            lyrics_line_text_view.setStringValue(ns_string!("..."));
        };
    }

    pub fn update_now_playing_info(
        &self,
        now_playing_info: Option<NowPlayingInfo>
    ) -> anyhow::Result<()> {
        *self.now_playing_info.lock().map_err(|_| anyhow!("Failed to obtain lock"))? = now_playing_info;
        *self.last_now_playing_info_update_timestamp.lock().map_err(|_| anyhow!("Failed to obtain lock"))? = Some(Instant::now());

        Ok(())
    }
}