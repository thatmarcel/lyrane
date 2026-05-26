use crate::now_playing_update_listener::NowPlayingUpdateListener;
use crate::now_playing_update_message::NowPlayingInfo;
use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchQueueGlobalPriority, GlobalQueueIdentifier};
use objc2::__framework_prelude::Retained;
use objc2_foundation::NSTimer;
use sonos_sdk::SonosSystem;
use std::cell::OnceCell;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, OnceLock};

const SONOS_NOW_PLAYING_UPDATE_TIME_INTERVAL: f64 = 1f64;

unsafe impl Send for SonosNowPlaying {}
unsafe impl Sync for SonosNowPlaying {}

pub struct SonosNowPlaying {
    timer: OnceCell<Retained<NSTimer>>,
    system: Arc<OnceLock<SonosSystem>>,
    prev_info: Arc<Mutex<Option<NowPlayingInfo>>>,
    prev_playback_rate: Arc<Mutex<f64>>
}

impl SonosNowPlaying {
    pub fn start_new_on_current_thread() -> anyhow::Result<Arc<Self>> {
        let self_arc = Arc::new(Self {
            timer: OnceCell::new(),
            system: Arc::new(OnceLock::new()),
            prev_info: Arc::new(Mutex::new(None)),
            prev_playback_rate: Arc::new(Mutex::new(-1f64)),
        });

        let self_arc_clone = self_arc.clone();

        DispatchQueue::global_queue(
            GlobalQueueIdentifier::Priority(DispatchQueueGlobalPriority::Default)
        ).exec_async(move || {
            self_arc_clone.system.set(
                SonosSystem::new()
                    .expect("Failed to create new Sonos system")
            ).map_err(|_|
                anyhow::anyhow!("Failed to set Sonos system")
            ).unwrap();
        });

        let self_arc_clone = self_arc.clone();

        unsafe {
            let timer = NSTimer::scheduledTimerWithTimeInterval_repeats_block(
                SONOS_NOW_PLAYING_UPDATE_TIME_INTERVAL,
                true,
                &RcBlock::new(move |_timer: NonNull<NSTimer>| {
                    self_arc_clone.run();
                })
            );

            self_arc.timer.set(timer)
                .map_err(|_| anyhow::anyhow!("Failed to set timer"))?;
        }

        Ok(self_arc)
    }

    fn run(&self) {
        let Some(system) = self.system.get() else { return; };
        let speakers = system.speakers();

        for speaker in speakers {
            if let Ok(current_track) = speaker.current_track.fetch() &&
               let Some(current_track_title) = current_track.title &&
               let Some(current_track_artist_name) = current_track.artist &&
               let Ok(current_position) = speaker.position.fetch() &&
               let Ok(current_playback_state) = speaker.playback_state.fetch()
            {
                let current_playback_rate = match current_playback_state.is_playing() {
                    true => 1f64,
                    false => 0f64
                };

                let now_playing_info = NowPlayingInfo {
                    title: current_track_title,
                    artist_name: current_track_artist_name,
                    current_position_seconds: ((((current_position.position_ms as f64) / 1000f64 * 2f64) as u64) as f64 / 2f64) - 0.2f64,
                    duration_seconds: (current_position.duration_ms as f64) / 1000f64,
                    playback_rate: current_playback_rate
                };

                let mut prev_info = self.prev_info.lock().unwrap();
                let mut prev_playback_rate = self.prev_playback_rate.lock().unwrap();

                let has_item_changed = !now_playing_info.is_same_item_as_in_option(&prev_info);
                let has_playback_rate_changed = current_playback_rate != *prev_playback_rate;

                *prev_info = Some(now_playing_info.clone());
                *prev_playback_rate = current_playback_rate;

                NowPlayingUpdateListener::call_now_playing_update_handler(
                    Some(now_playing_info),
                    has_item_changed,
                    has_playback_rate_changed,
                    true
                );

                return;
            }
        }

        NowPlayingUpdateListener::call_now_playing_update_handler(
            None,
            true,
            true,
            true
        );
    }
}