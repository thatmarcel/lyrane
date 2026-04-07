use crate::app::LyraneAppDelegate;
use crate::now_playing_update_message::{NowPlayingInfo, NowPlayingUpdateMessage};
use anyhow::anyhow;
use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;
use std::io::Read;
use std::process::{Command, Stdio};

pub struct NowPlayingUpdateListener {}

impl NowPlayingUpdateListener {
    pub fn start_on_current_thread(script_file_path_str: &str) -> anyhow::Result<()> {
        let mut script_handle = Command::new("osascript")
            .arg(script_file_path_str)
            .stderr(Stdio::piped())
            .spawn()?;

        // It's easier to write to stderr from AppleScript
        let mut script_output_pipe = script_handle.stderr.take().ok_or(
            anyhow!("Failed to get stderr of spawned osascript process")
        )?;

        let mut script_output_line_buffer = [0; 1024];

        let mut prev_info: Option<NowPlayingInfo> = None;
        let mut prev_playback_rate = -1f64;

        while let Ok(script_output_number_of_bytes_read) = script_output_pipe.read(&mut script_output_line_buffer) {
            let script_output_line_string = String::from_utf8_lossy(
                &script_output_line_buffer[..script_output_number_of_bytes_read]
            ).to_string();
            let script_output_line_string = script_output_line_string.trim();

            let msg: NowPlayingUpdateMessage = match serde_json::from_str(&script_output_line_string) {
                Ok(m) => m,
                Err(e) => {
                    println!("{:?}", e);
                    continue;
                }
            };

            match msg {
                NowPlayingUpdateMessage::Empty {} => {
                    if prev_info.is_none() {
                        continue;
                    }

                    Self::call_now_playing_update_handler(
                        None,
                        true,
                        true
                    );

                    prev_info = None;

                    continue;
                },
                NowPlayingUpdateMessage::WithInfo(now_playing_info) => {
                    let has_item_changed = !now_playing_info.is_same_item_as_in_option(&prev_info);
                    let has_playback_rate_changed = prev_playback_rate != now_playing_info.playback_rate;

                    prev_info = Some(now_playing_info.clone());
                    prev_playback_rate = now_playing_info.playback_rate;

                    Self::call_now_playing_update_handler(
                        Some(now_playing_info),
                        has_item_changed,
                        has_playback_rate_changed
                    )
                }
            }
        }

        script_handle.wait()?;

        Ok(())
    }

    fn call_now_playing_update_handler(
        now_playing_info: Option<NowPlayingInfo>,
        has_item_changed: bool,
        has_playback_rate_changed: bool
    ) {
        DispatchQueue::main().exec_async(move || {
            let app = NSApplication::sharedApplication(
                MainThreadMarker::new().expect("DispatchQueue.main was not on main thread for some reason")
            );

            let app_delegate: Retained<LyraneAppDelegate> = app.delegate()
                .expect("Failed to get app delegate")
                .downcast::<LyraneAppDelegate>()
                .expect("Failed to cast app delegate to our struct");

            if let Err(error) = app_delegate.handle_now_playing_update(
                now_playing_info,
                has_item_changed,
                has_playback_rate_changed
            ) {
                println!("{:?}", error);
            }
        });
    }
}