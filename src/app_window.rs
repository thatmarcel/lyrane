use std::sync::atomic::{AtomicBool, Ordering};
use objc2::{define_class, msg_send, ClassType, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2::rc::{Retained};
use objc2::runtime::{AnyObject, NSObjectProtocol};
use objc2_app_kit::{NSApplication, NSBackingStoreType, NSColor, NSEvent, NSTrackingArea, NSTrackingAreaOptions, NSWindow, NSWindowStyleMask};
use objc2_foundation::{ns_string, NSClassFromString, NSNumber, NSRect};
use objc2_quartz_core::{CABasicAnimation, CAMediaTiming, CATransaction};
use crate::app::LyraneAppDelegate;
use crate::persistent_stored_config::{store_persistent_config_from_app_window};

#[derive(Debug, Default)]
pub struct AppWindowIvars {
    pub has_had_mouse_entered: AtomicBool
}

define_class!(
    #[derive(Debug)]
    #[unsafe(super = NSWindow)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppWindowIvars]
    pub struct AppWindow;

    unsafe impl NSObjectProtocol for AppWindow {}

    impl AppWindow {
        // Make sure the tracking area responsible for the mouse entered and exited events resizes with the window
        #[unsafe(method(_reallySetFrame:))]
        fn really_set_frame(&self, frame: NSRect) {
            let _: () = unsafe {
                msg_send![super(self), _reallySetFrame: frame]
            };

            let Some(content_view) = self.contentView() else {
                return;
            };

            content_view.trackingAreas().iter().for_each(|ta| content_view.removeTrackingArea(&ta));

            unsafe {
                content_view.addTrackingArea(&NSTrackingArea::initWithRect_options_owner_userInfo(
                    msg_send![NSTrackingArea::class(), alloc],
                    content_view.frame(),
                    NSTrackingAreaOptions::ActiveAlways | NSTrackingAreaOptions::MouseEnteredAndExited,
                    Some(&self),
                    None
                ));
            }

            if !self.inLiveResize() && self.ivars().has_had_mouse_entered.load(Ordering::SeqCst) {
                store_persistent_config_from_app_window(&self);
            }
        }

        // Make tiling (by dragging the window to a corner etc.) do nothing because it is annoying when
        // moving the window onto the menu bar, for example
        #[unsafe(method(_resizeFromWindowManagerWithTargetGeometry:springSettings:completion:))]
        fn resize_from_window_manager_with_target_geometry(
            &self,
            target_geometry: &AnyObject /* _WMResizeTargetGeometry */,
            spring_settings: &AnyObject /* _WMSpringAnimationSettings */,
            completion: &AnyObject /* __NSStackBlock__ */
        ) {
            unsafe {
                #[allow(non_snake_case)]
                let modified_target_geometry = match NSClassFromString(ns_string!("_WMResizeTargetGeometry")) {
                    Some(_WMResizeTargetGeometry) => msg_send![_WMResizeTargetGeometry, targetGeometryWithSize: self.frame().size],
                    None => target_geometry
                };

                msg_send![
                    super(self),
                    _resizeFromWindowManagerWithTargetGeometry: modified_target_geometry,
                    springSettings: spring_settings,
                    completion: completion
                ]
            }
        }

        #[unsafe(method(_shouldShowResizeCursor))]
        fn should_show_resize_cursor(&self) -> bool {
            true
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            let app_delegate: Retained<LyraneAppDelegate> = NSApplication::sharedApplication(self.mtm())
                .delegate()
                .expect("Failed to get app delegate")
                .downcast()
                .expect("Failed to downcast app delegate");

            app_delegate.handle_right_click(event, &self.contentView().unwrap());
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            self.ivars().has_had_mouse_entered.store(true, Ordering::SeqCst);

            let Some(content_view_layer) = self.contentView().and_then(|content_view| content_view.layer()) else { return; };

            content_view_layer.removeAnimationForKey(ns_string!("mouse-exited-border-width-animation"));
            content_view_layer.removeAnimationForKey(ns_string!("mouse-exited-border-color-animation"));

            unsafe {
                CATransaction::begin();

                let animation = CABasicAnimation::animationWithKeyPath(Some(ns_string!("borderWidth")));
                animation.setFromValue(Some(&NSNumber::new_f64(0f64)));
                animation.setToValue(Some(&NSNumber::new_f64(1.5f64)));
                animation.setDuration(0.1f64);
                content_view_layer.addAnimation_forKey(&animation, Some(ns_string!("mouse-entered-border-width-animation")));

                let animation2 = CABasicAnimation::animationWithKeyPath(Some(ns_string!("borderColor")));
                animation2.setFromValue(Some(&NSColor::clearColor().CGColor().as_ref()));
                animation2.setToValue(Some(&NSColor::whiteColor().CGColor().as_ref()));
                animation2.setDuration(0.1f64);
                content_view_layer.addAnimation_forKey(&animation2, Some(ns_string!("mouse-entered-border-color-animation")));

                content_view_layer.setBorderWidth(1.5f64);
                content_view_layer.setBorderColor(Some(&NSColor::whiteColor().CGColor()));

                CATransaction::commit();
            }
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            let Some(content_view_layer) = self.contentView().and_then(|content_view| content_view.layer()) else { return; };

            content_view_layer.removeAnimationForKey(ns_string!("mouse-entered-border-width-animation"));
            content_view_layer.removeAnimationForKey(ns_string!("mouse-entered-border-color-animation"));

            unsafe {
                CATransaction::begin();

                let animation = CABasicAnimation::animationWithKeyPath(Some(ns_string!("borderWidth")));
                animation.setFromValue(Some(&NSNumber::new_f64(1.5f64)));
                animation.setToValue(Some(&NSNumber::new_f64(0f64)));
                animation.setDuration(0.1f64);
                content_view_layer.addAnimation_forKey(&animation, Some(ns_string!("mouse-exited-border-width-animation")));

                let animation2 = CABasicAnimation::animationWithKeyPath(Some(ns_string!("borderColor")));
                animation2.setFromValue(Some(&NSColor::whiteColor().CGColor().as_ref()));
                animation2.setToValue(Some(&NSColor::clearColor().CGColor().as_ref()));
                animation2.setDuration(0.1f64);
                content_view_layer.addAnimation_forKey(&animation2, Some(ns_string!("mouse-exited-border-color-animation")));

                content_view_layer.setBorderWidth(0f64);
                content_view_layer.setBorderColor(Some(&NSColor::clearColor().CGColor()));

                CATransaction::commit();
            }
        }
    }
);

impl AppWindow {
    pub fn new_with_content_rect_style_mask_backing_defer(
        mtm: MainThreadMarker,
        content_rect: NSRect,
        style: NSWindowStyleMask,
        backing_store_type: NSBackingStoreType,
        defer: bool
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AppWindowIvars::default());

        unsafe {
            msg_send![
                super(this),
                initWithContentRect: content_rect,
                styleMask: style,
                backing: backing_store_type,
                defer: defer
            ]
        }
    }
}