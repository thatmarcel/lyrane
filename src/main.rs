use crate::app::launch_app_kit_app;
use anyhow::anyhow;
use objc2::MainThreadMarker;
use crate::persistent_stored_config::create_lock_file;

mod now_playing_update_message;
mod now_playing_update_listener;
mod app;
mod create_app_window;
mod now_playing_update_script;
mod lyrics_fetcher;
mod lyrics_line;
mod lyrics_syncer;
mod app_window;
mod persistent_stored_config;

fn main() -> anyhow::Result<()> {
    let Some(lock_file) = create_lock_file() else {
        println!("App is already running (failed to create lock file), exiting");

        return Ok(());
    };

    launch_app_kit_app(MainThreadMarker::new().ok_or(anyhow!("Main thread check failed"))?, lock_file);

    Ok(())
}
