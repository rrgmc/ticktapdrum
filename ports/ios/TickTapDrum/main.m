// The app's entry point. It sets up the audio session, works out the app's two folders and hands
// over to Rust, which runs the interface through eframe and winit.

#import <AVFoundation/AVFoundation.h>
#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <objc/runtime.h>

#import "ticktapdrum.h"

// Apps built with the iOS 27 SDK must use scenes: without a scene manifest UIKit refuses to
// launch them. winit 0.30, which eframe uses, predates that and creates its UIWindow with no
// scene, which then never shows. The Info.plist declares one scene with DMSceneDelegate; Rust
// passes winit's view to dm_ios_attach_view on its first frame; whichever of the two comes second
// gives the window to the scene.
@interface DMSceneDelegate : UIResponder <UIWindowSceneDelegate>
@property(nonatomic, strong) UIWindow *window;
@end

static UIWindowScene *dm_scene = nil;
static UIWindow *dm_window = nil;

// winit 0.30 reports the safe area as the window's inner size on iOS. eframe 0.37 sizes its
// drawing surface to that inner size, while egui lays out the whole window (its outer size), so
// the picture was stretched over the full screen and every tap landed off target: by an eighth
// of the height on an iPhone with a Dynamic Island. winit's window class is told there are no
// insets, so both sizes are the full window. The app keeps clear of the notch and the home
// indicator itself, with the real insets from dm_ios_safe_area. Only winit's own UIWindow
// subclass changes, never UIWindow.
static UIEdgeInsets (*dm_real_insets)(id, SEL) = NULL;

static UIEdgeInsets dm_no_insets(id self, SEL cmd) {
    (void)self;
    (void)cmd;
    return UIEdgeInsetsZero;
}

static void dm_hide_insets_from_winit(UIWindow *window) {
    if (dm_real_insets != NULL) {
        return;
    }
    Class winit_class = object_getClass(window);
    if (winit_class == [UIWindow class]) {
        NSLog(@"ticktapdrum: winit's window is a plain UIWindow; leaving its insets alone");
        return;
    }
    SEL selector = @selector(safeAreaInsets);
    Method inherited = class_getInstanceMethod(winit_class, selector);
    dm_real_insets = (UIEdgeInsets(*)(id, SEL))method_getImplementation(inherited);
    class_replaceMethod(winit_class, selector, (IMP)dm_no_insets,
                        method_getTypeEncoding(inherited));
}

void dm_ios_keep_awake(int on) {
    UIApplication.sharedApplication.idleTimerDisabled = on != 0;
}

int dm_ios_is_ipad(void) {
    return UIDevice.currentDevice.userInterfaceIdiom == UIUserInterfaceIdiomPad;
}

void dm_ios_safe_area(double *top, double *left, double *bottom, double *right) {
    UIEdgeInsets insets = UIEdgeInsetsZero;
    if (dm_window != nil && dm_real_insets != NULL) {
        insets = dm_real_insets(dm_window, @selector(safeAreaInsets));
    }
    // On a device without a notch the window has no insets: the status bar counts only in the
    // root view's safe area.
    UIEdgeInsets root = dm_window.rootViewController.view.safeAreaInsets;
    insets.top = MAX(insets.top, root.top);
    insets.left = MAX(insets.left, root.left);
    insets.bottom = MAX(insets.bottom, root.bottom);
    insets.right = MAX(insets.right, root.right);
    // From iPadOS 26 an app can run in a window whose controls (close, minimise, resize) sit
    // in its top-left corner, inside the plain safe area. The corner-adapted region keeps clear
    // of them; vertical adaptation moves the top row down rather than narrowing every row.
    if (@available(iOS 26.0, *)) {
        UIView *view = dm_window.rootViewController.view;
        if (view != nil) {
            UIEdgeInsets corner = [view
                edgeInsetsForLayoutRegion:[UIViewLayoutRegion
                                              safeAreaLayoutRegionWithCornerAdaptation:
                                                  UIViewLayoutRegionAdaptivityAxisVertical]];
            insets.top = MAX(insets.top, corner.top);
            insets.left = MAX(insets.left, corner.left);
            insets.bottom = MAX(insets.bottom, corner.bottom);
            insets.right = MAX(insets.right, corner.right);
        }
    }
    *top = insets.top;
    *left = insets.left;
    *bottom = insets.bottom;
    *right = insets.right;
}

static void dm_join(void) {
    if (dm_scene == nil || dm_window == nil || dm_window.windowScene == dm_scene) {
        return;
    }
    dm_window.windowScene = dm_scene;
    [dm_window makeKeyAndVisible];
    NSLog(@"ticktapdrum: gave the window to the scene");
    // eframe sized its surface before the insets were hidden. A fresh layout makes winit report
    // a resize, and eframe then sizes the surface to the full window. Asynchronous, because this
    // runs inside eframe's own start-up.
    UIView *view = dm_window.rootViewController.view;
    dispatch_async(dispatch_get_main_queue(), ^{
        [view setNeedsLayout];
        [view layoutIfNeeded];
    });
}

void dm_ios_attach_view(void *view) {
    dm_window = ((__bridge UIView *)view).window;
    if (dm_window == nil) {
        NSLog(@"ticktapdrum: the view has no window");
        return;
    }
    dm_hide_insets_from_winit(dm_window);
    dm_join();
}

@implementation DMSceneDelegate

- (void)scene:(UIScene *)scene
    willConnectToSession:(UISceneSession *)session
                 options:(UISceneConnectionOptions *)connectionOptions {
    dm_scene = (UIWindowScene *)scene;
    dm_join();
}

@end

// Creates `path` if needed. Returns it, or nil when it cannot be made.
static NSString *dm_ensure(NSString *path) {
    NSError *error = nil;
    if (![[NSFileManager defaultManager] createDirectoryAtPath:path
                                   withIntermediateDirectories:YES
                                                    attributes:nil
                                                         error:&error]) {
        NSLog(@"ticktapdrum: could not create %@: %@", path, error);
        return nil;
    }
    return path;
}

static NSString *dm_folder(NSSearchPathDirectory directory, NSString *child) {
    NSArray<NSString *> *roots =
        NSSearchPathForDirectoriesInDomains(directory, NSUserDomainMask, YES);
    if (roots.count == 0) {
        return nil;
    }
    return dm_ensure([roots.firstObject stringByAppendingPathComponent:child]);
}

// cpal's CoreAudio backend sets up no audio session. Without one the app gets the soloAmbient
// category: the ringer switch mutes it and nothing tells the app. Playback plays through the
// switch. No background audio mode is declared, so the sound still stops when the app leaves the
// screen.
static void dm_configure_audio_session(void) {
    AVAudioSession *session = [AVAudioSession sharedInstance];
    NSError *error = nil;
    if (![session setCategory:AVAudioSessionCategoryPlayback error:&error]) {
        NSLog(@"ticktapdrum: could not set the audio category: %@", error);
        return;
    }
    // iOS picks a buffer of about 23 ms by default (1024 frames at 44.1 kHz on an iPad), which
    // adds to the delay between a tap and its sound. Ask for 5 ms; iOS rounds it to what the
    // hardware allows.
    if (![session setPreferredIOBufferDuration:0.005 error:&error]) {
        NSLog(@"ticktapdrum: could not ask for a shorter audio buffer: %@", error);
    }
    if (![session setActive:YES error:&error]) {
        NSLog(@"ticktapdrum: could not activate the audio session: %@", error);
    }
}

int main(int argc, char *argv[]) {
    (void)argc;
    (void)argv;
    NSString *data = nil;
    NSString *exports = nil;
    @autoreleasepool {
        dm_configure_audio_session();
        // The bank, the projects and the autosave: private to the app, kept by backups.
        data = dm_folder(NSApplicationSupportDirectory, @"ticktapdrum");
        // Exports: under Documents, which the Files app shows because of UIFileSharingEnabled.
        exports = dm_folder(NSDocumentDirectory, @"exports");
        if (data == nil || exports == nil) {
            NSLog(@"ticktapdrum: no data or export folder, stopping");
            return 1;
        }
        // Rust copies both strings before the pool drains.
        dm_ios_main(data.fileSystemRepresentation, exports.fileSystemRepresentation);
    }
    return 1;
}
