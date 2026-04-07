use crate::app_window::AppWindow;
use anyhow::anyhow;
use objc2::rc::Retained;
use objc2::{msg_send, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSBackingStoreType, NSColor, NSLayoutConstraint, NSTrackingArea, NSTrackingAreaOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectView, NSWindowAnimationBehavior, NSWindowCollectionBehavior, NSWindowStyleMask};
use objc2_foundation::{ns_string, NSArray, NSPoint, NSRect, NSSize};

const INITIAL_WINDOW_SIZE_WIDTH: f64 = 400f64;
const INITIAL_WINDOW_SIZE_HEIGHT: f64 = 100f64;

const MIN_WINDOW_SIZE_WIDTH: f64 = 300f64;
const MIN_WINDOW_SIZE_HEIGHT: f64 = 10f64;

pub fn create_app_window(mtm: MainThreadMarker) -> anyhow::Result<(Retained<AppWindow>, Retained<NSVisualEffectView>)> {
    unsafe {
        let window = AppWindow::new_with_content_rect_style_mask_backing_defer(
            mtm,
            NSRect::new(
                NSPoint::ZERO,
                NSSize::new(INITIAL_WINDOW_SIZE_WIDTH, INITIAL_WINDOW_SIZE_HEIGHT)
            ),
            NSWindowStyleMask::Borderless |
                NSWindowStyleMask::Resizable |
                NSWindowStyleMask::NonactivatingPanel,
            NSBackingStoreType::Buffered,
            false
        );

        window.setReleasedWhenClosed(false);

        window.setTitle(ns_string!("Lyrane"));

        let content_view = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(
                NSPoint::ZERO,
                NSSize::new(INITIAL_WINDOW_SIZE_WIDTH, INITIAL_WINDOW_SIZE_HEIGHT)
            )
        );

        content_view.addTrackingArea(&NSTrackingArea::initWithRect_options_owner_userInfo(
            msg_send![NSTrackingArea::class(), alloc],
            content_view.bounds(),
            NSTrackingAreaOptions::ActiveAlways | NSTrackingAreaOptions::MouseEnteredAndExited,
            Some(&window),
            None
        ));

        let effect_view = NSVisualEffectView::initWithFrame(
            NSVisualEffectView::alloc(mtm),
            NSRect::new(
                NSPoint::ZERO,
                NSSize::new(INITIAL_WINDOW_SIZE_WIDTH, INITIAL_WINDOW_SIZE_HEIGHT)
            )
        );

        effect_view.setTranslatesAutoresizingMaskIntoConstraints(false);

        effect_view.setWantsLayer(true);
        effect_view.layer().ok_or(anyhow!("Effect view is missing layer"))?.setOpacity(0.95f32);

        effect_view.setMaterial(NSVisualEffectMaterial::WindowBackground);
        effect_view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);

        content_view.setWantsLayer(true);

        content_view.setClipsToBounds(true);

        let content_view_layer = content_view.layer().ok_or(anyhow!("Content view is missing layer"))?;

        // If the background is set to a completely transparent color, the view seems to get ignored when calculating the window's bounds
        // and having the window keep its size etc. gets a bit more difficult
        content_view_layer.setBackgroundColor(Some(&NSColor::blackColor().colorWithAlphaComponent(0.001f64).CGColor()));

        content_view_layer.setCornerRadius(8f64);
        content_view_layer.setMasksToBounds(true);

        window.setContentSize(NSSize::new(INITIAL_WINDOW_SIZE_WIDTH, INITIAL_WINDOW_SIZE_HEIGHT));

        window.setContentView(Some(&content_view));

        content_view.addSubview(&effect_view);

        NSLayoutConstraint::activateConstraints(&NSArray::from_slice(&[
            &*effect_view.topAnchor().constraintEqualToAnchor(&content_view.topAnchor()),
            &*effect_view.bottomAnchor().constraintEqualToAnchor(&content_view.bottomAnchor()),
            &*effect_view.leftAnchor().constraintEqualToAnchor(&content_view.leftAnchor()),
            &*effect_view.rightAnchor().constraintEqualToAnchor(&content_view.rightAnchor()),
        ]));

        window.setMinSize(NSSize::new(MIN_WINDOW_SIZE_WIDTH, MIN_WINDOW_SIZE_HEIGHT));

        window.setBackgroundColor(Some(&NSColor::clearColor()));

        window.setMovableByWindowBackground(true);

        window.setLevel(100000isize);

        window.makeKeyAndOrderFront(None);

        window.setRestorable(false);

        window.setCollectionBehavior(
            NSWindowCollectionBehavior::FullScreenNone |
                NSWindowCollectionBehavior::IgnoresCycle |
                NSWindowCollectionBehavior::Transient
        );

        window.setAnimationBehavior(NSWindowAnimationBehavior::None);

        Ok((window.downcast().unwrap(), effect_view))
    }
}