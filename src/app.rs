use std::cell::OnceCell;
use std::fs::File;
use std::sync::Arc;
use anyhow::anyhow;
use dispatch2::{DispatchQueue, DispatchQueueGlobalPriority, GlobalQueueIdentifier};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSColor, NSEvent, NSFont, NSLayoutConstraint, NSLayoutConstraintOrientation, NSLayoutPriorityDefaultLow, NSMenu, NSMenuItem, NSScreen, NSTextAlignment, NSTextField, NSView, NSVisualEffectView, NSWindowDelegate};
use objc2_foundation::{ns_string, NSArray, NSNotification, NSRect};
use crate::app_window::AppWindow;
use crate::create_app_window::create_app_window;
use crate::lyrics_fetcher::fetch_lyrics;
use crate::lyrics_syncer::LyricsSyncer;
use crate::now_playing_update_listener::NowPlayingUpdateListener;
use crate::now_playing_update_message::{NowPlayingInfo};
use crate::now_playing_update_script::create_now_playing_update_script_file;
use crate::persistent_stored_config::{load_persistent_config, remove_lock_file, store_persistent_config_from_app_window};

#[derive(Debug)]
pub struct LyraneAppDelegateIvars {
    lock_file: Arc<File>,
    window: OnceCell<Retained<AppWindow>>,
    window_effect_view: OnceCell<Retained<NSVisualEffectView>>,
    lyrics_line_text_view: OnceCell<Retained<NSTextField>>,
    temp_script_file_path_string: OnceCell<String>,
    lyrics_syncer: OnceCell<Arc<LyricsSyncer>>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = LyraneAppDelegateIvars]
    pub struct LyraneAppDelegate;

    unsafe impl NSObjectProtocol for LyraneAppDelegate {}

    unsafe impl NSApplicationDelegate for LyraneAppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, notification: &NSNotification) {
            let app = notification.object()
                .unwrap()
                .downcast::<NSApplication>()
                .unwrap();

            self.handle_finish_launching(app).unwrap();
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            if let Some(temp_script_file_path_string) = self.ivars().temp_script_file_path_string.get() {
                let _ = std::fs::remove_file(temp_script_file_path_string);
                remove_lock_file(&self.ivars().lock_file);
            };
        }
    }

    unsafe impl NSWindowDelegate for LyraneAppDelegate {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            NSApplication::sharedApplication(self.mtm()).terminate(None);
        }
    }

    impl LyraneAppDelegate {
        #[unsafe(method(handleMenuItemToggleBackgroundClick))]
        fn handle_menu_item_toggle_background_click(&self) {
            let app_window_effect_view = self.ivars()
                .window_effect_view
                .get()
                .expect("Failed to get app window effect view");

            self.set_background_enabled(app_window_effect_view.isHidden());
        }

        #[unsafe(method(handleMenuItemQuitClick))]
        fn handle_menu_item_quit_click(&self) {
            if let Some(app_window) = self.ivars().window.get() {
                app_window.close();
            } else {
                NSApplication::sharedApplication(self.mtm()).terminate(None);
            }
        }

        #[unsafe(method(handleMenuItemSetTextAlignmentLeftClick))]
        fn handle_menu_item_set_text_alignment_left_click(&self) {
            self.set_lyrics_line_text_alignment(NSTextAlignment::Left);
        }

        #[unsafe(method(handleMenuItemSetTextAlignmentCenterClick))]
        fn handle_menu_item_set_text_alignment_center_click(&self) {
            self.set_lyrics_line_text_alignment(NSTextAlignment::Center);
        }

        #[unsafe(method(handleMenuItemSetTextAlignmentRightClick))]
        fn handle_menu_item_set_text_alignment_right_click(&self) {
            self.set_lyrics_line_text_alignment(NSTextAlignment::Right);
        }
    }
);

impl LyraneAppDelegate {
    pub fn new(mtm: MainThreadMarker, lock_file: File) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(LyraneAppDelegateIvars {
            lock_file: Arc::new(lock_file),
            window: Default::default(),
            window_effect_view: Default::default(),
            lyrics_line_text_view: Default::default(),
            temp_script_file_path_string: Default::default(),
            lyrics_syncer: Default::default()
        });

        unsafe {
            msg_send![super(this), init]
        }
    }

    fn handle_finish_launching(&self, app: Retained<NSApplication>) -> anyhow::Result<()> {
        let mtm = self.mtm();

        let (app_window, app_window_effect_view) = create_app_window(mtm)?;

        app_window.setDelegate(Some(ProtocolObject::from_ref(self)));

        self.setup_views(&app_window)?;

        app_window.setIsVisible(false);

        self.ivars().window.set(app_window)
            .map_err(|_| anyhow!("The application's window has already been set"))?;

        self.ivars().window_effect_view.set(app_window_effect_view)
            .map_err(|_| anyhow!("The application's window effect view has already been set"))?;

        let app_window = self.ivars().window.get()
            .ok_or(anyhow!("The application's window isn't set even though we just set it"))?;

        if let Some(config) = load_persistent_config() {
            let main_screen = NSScreen::mainScreen(mtm).ok_or(anyhow!("Failed to get main screen"))?;

            if config.previous_screen_size == main_screen.frame().size.into() {
                app_window.setFrame_display(config.previous_window_frame.into(), true);
            } else {
                app_window.center();
            }

            self.set_background_enabled(config.previously_had_background_enabled);

            self.set_lyrics_line_text_alignment(config.previous_text_alignment.into());
        } else {
            app_window.center();
        }

        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);

        let temp_script_file_path_string = create_now_playing_update_script_file()?;

        self.ivars().temp_script_file_path_string.set(temp_script_file_path_string.clone())
            .map_err(|_| anyhow!("The application's now playing update script file path has already been set"))?;

        let temp_script_file_path_string_clone_for_ctrlc_handler = temp_script_file_path_string.clone();
        let lock_file_clone_for_ctrlc_handler = self.ivars().lock_file.clone();
        ctrlc::set_handler(move || {
            let _ = std::fs::remove_file(&temp_script_file_path_string_clone_for_ctrlc_handler);
            remove_lock_file(&lock_file_clone_for_ctrlc_handler);
            std::process::exit(0i32);
        })?;

        DispatchQueue::global_queue(
            GlobalQueueIdentifier::Priority(DispatchQueueGlobalPriority::Default)
        ).exec_async(move || {
            NowPlayingUpdateListener::start_on_current_thread(&temp_script_file_path_string)
                .expect("Failed to start now playing update listener");
        });

        self.ivars().lyrics_syncer.set(
            LyricsSyncer::new(
                self.ivars()
                    .lyrics_line_text_view
                    .get()
                    .ok_or(anyhow!("Failed to get lyrics line text view"))?
            )?
        ).map_err(|_| anyhow!("The lyric syncer has already been set"))?;

        Ok(())
    }

    pub fn setup_views(&self, app_window: &Retained<AppWindow>) -> anyhow::Result<()> {
        let mtm = self.mtm();

        let lyrics_line_text_view = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::ZERO
        );

        lyrics_line_text_view.setTranslatesAutoresizingMaskIntoConstraints(false);

        let app_window_content_view = app_window.contentView()
            .ok_or(anyhow!("Failed to get window content view"))?;

        app_window_content_view.addSubview(&lyrics_line_text_view);

        lyrics_line_text_view.setContentCompressionResistancePriority_forOrientation(
            NSLayoutPriorityDefaultLow,
            NSLayoutConstraintOrientation::Vertical
        );

        lyrics_line_text_view.setContentCompressionResistancePriority_forOrientation(
            NSLayoutPriorityDefaultLow,
            NSLayoutConstraintOrientation::Horizontal
        );

        NSLayoutConstraint::activateConstraints(&*NSArray::from_slice(&[
            &*lyrics_line_text_view.centerYAnchor()
                .constraintEqualToAnchor(
                    &app_window_content_view.centerYAnchor()
                ),
            &*lyrics_line_text_view.leftAnchor()
                .constraintEqualToAnchor_constant(
                    &app_window_content_view.leftAnchor(),
                    8f64
                ),
            &*lyrics_line_text_view.rightAnchor()
                .constraintEqualToAnchor_constant(
                    &app_window_content_view.rightAnchor(),
                    -8f64
                ),
            &*lyrics_line_text_view.heightAnchor()
                .constraintLessThanOrEqualToAnchor(
                    &app_window_content_view.heightAnchor()
                )
        ]));

        lyrics_line_text_view.setBordered(false);

        lyrics_line_text_view.setBackgroundColor(Some(&NSColor::clearColor()));

        lyrics_line_text_view.setWantsLayer(true);

        lyrics_line_text_view.layer()
            .ok_or(anyhow!("Lyrics text view is missing layer"))?
            .setMasksToBounds(true);
        lyrics_line_text_view.setClipsToBounds(true);

        lyrics_line_text_view.setTextColor(Some(&NSColor::labelColor()));
        lyrics_line_text_view.setAlignment(NSTextAlignment::Center);

        lyrics_line_text_view.setFont(Some(&NSFont::boldSystemFontOfSize(16f64)));
        lyrics_line_text_view.setStringValue(ns_string!(""));

        lyrics_line_text_view.setSelectable(false);
        lyrics_line_text_view.setEditable(false);

        self.ivars().lyrics_line_text_view.set(lyrics_line_text_view)
            .map_err(|_| anyhow!("The lyrics line text view has already been set"))?;

        Ok(())
    }

    pub fn handle_now_playing_update(
        &self,
        now_playing_info: Option<NowPlayingInfo>,
        has_item_changed: bool,
        _has_playback_rate_changed: bool
    ) -> anyhow::Result<()> {
        let app_window = self.ivars().window.get().ok_or(anyhow!("Failed to get app window"))?;

        let lyrics_syncer = self.ivars().lyrics_syncer.get().ok_or(anyhow!("Failed to get lyrics syncer"))?;

        lyrics_syncer.update_now_playing_info(now_playing_info.clone())?;

        let lyrics_syncer_lyrics_arc = lyrics_syncer.lyrics.clone();
        let lyrics_syncer_now_playing_item_arc = lyrics_syncer.now_playing_info.clone();

        if let Some(now_playing_info) = now_playing_info {
            if has_item_changed {
                // println!("{:?} is now playing", now_playing_info);

                *lyrics_syncer_lyrics_arc.lock().expect("Failed to obtain lock") = None;

                app_window.setIsVisible(false);

                DispatchQueue::global_queue(GlobalQueueIdentifier::Priority(DispatchQueueGlobalPriority::Default)).exec_async(move || {
                    if let Ok(lyrics) = fetch_lyrics(&now_playing_info.title, &now_playing_info.artist_name) {
                        let lyrics_syncer_now_playing_item = &*lyrics_syncer_now_playing_item_arc
                            .lock()
                            .expect("Failed to obtain lock");

                        if now_playing_info.is_same_item_as_in_option(lyrics_syncer_now_playing_item) {
                            *lyrics_syncer_lyrics_arc
                                .lock()
                                .expect("Failed to obtain lock") = Some(lyrics);
                        }
                    }
                });
            } else {
                if let Ok(lyrics) = lyrics_syncer_lyrics_arc.lock() && lyrics.is_some() {
                    app_window.setIsVisible(now_playing_info.playback_rate > 0f64);
                }
            }
        } else {
            app_window.setIsVisible(false);
        }

        Ok(())
    }

    pub fn handle_right_click(&self, event: &NSEvent, view: &NSView) {
        let menu = NSMenu::initWithTitle(
            NSMenu::alloc(self.mtm()),
            ns_string!("Lyrane")
        );

        unsafe {
            menu.addItem(&NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(self.mtm()),
                ns_string!("Toggle background"),
                Some(sel!(handleMenuItemToggleBackgroundClick)),
                ns_string!("")
            ));

            menu.addItem(&*{
                let menu_item = NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(self.mtm()),
                    ns_string!("Set text alignment"),
                    None,
                    ns_string!("")
                );

                let sub_menu = NSMenu::initWithTitle(
                    NSMenu::alloc(self.mtm()),
                    &menu_item.title()
                );

                sub_menu.addItem(&NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(self.mtm()),
                    ns_string!("Left"),
                    Some(sel!(handleMenuItemSetTextAlignmentLeftClick)),
                    ns_string!("")
                ));

                sub_menu.addItem(&NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(self.mtm()),
                    ns_string!("Center"),
                    Some(sel!(handleMenuItemSetTextAlignmentCenterClick)),
                    ns_string!("")
                ));

                sub_menu.addItem(&NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(self.mtm()),
                    ns_string!("Right"),
                    Some(sel!(handleMenuItemSetTextAlignmentRightClick)),
                    ns_string!("")
                ));

                menu_item.setSubmenu(Some(&sub_menu));

                menu_item
            });

            menu.addItem(&NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(self.mtm()),
                ns_string!("Quit"),
                Some(sel!(handleMenuItemQuitClick)),
                ns_string!("")
            ));
        }

        NSMenu::popUpContextMenu_withEvent_forView(&*menu, event, view);
    }

    fn set_background_enabled(&self, should_enable_background: bool) {
        let app_window = self.ivars().window.get()
            .expect("Failed to get app window");
        let app_window_effect_view = self.ivars().window_effect_view.get()
            .expect("Failed to get app window effect view");
        let lyrics_line_text_view = self.ivars().lyrics_line_text_view.get()
            .expect("Failed to get lyrics line text view");

        if should_enable_background {
            app_window_effect_view.setHidden(false);

            lyrics_line_text_view.setTextColor(Some(&NSColor::labelColor()));
        } else {
            app_window_effect_view.setHidden(true);

            lyrics_line_text_view.setTextColor(Some(&NSColor::whiteColor()));
        }

        store_persistent_config_from_app_window(app_window);
    }

    fn set_lyrics_line_text_alignment(&self, alignment: NSTextAlignment) {
        let app_window = self.ivars().window.get()
            .expect("Failed to get app window");
        let lyrics_line_text_view = self.ivars().lyrics_line_text_view.get()
            .expect("Failed to get lyrics line text view");

        lyrics_line_text_view.setAlignment(alignment);

        store_persistent_config_from_app_window(app_window);
    }
}

pub fn launch_app_kit_app(mtm: MainThreadMarker, lock_file: File) {
    let app = NSApplication::sharedApplication(mtm);
    let app_delegate = LyraneAppDelegate::new(mtm, lock_file);

    app.setDelegate(Some(ProtocolObject::from_ref(&*app_delegate)));

    app.run();
}