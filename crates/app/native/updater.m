// The Mac updater: Sparkle, loaded from the app bundle when the bundle carries it. Sparkle is found at run time, not
// at link time, so a build from source (which has no Sparkle) runs and says it cannot update. The Rust side is
// src/updater/impls/sparkle_driver.rs.
#import <AppKit/AppKit.h>
#import <objc/message.h>
#include <stdbool.h>

typedef void (*atelier_relaunch_callback)(void);

static atelier_relaunch_callback relaunch_callback;
static id controller;
static id updater_delegate;
static void (^pending_install)(void);

@interface AtelierUpdaterDelegate : NSObject
@end

@implementation AtelierUpdaterDelegate
// Sparkle has an update ready and wants to restart the app. The app answers: atelier_updater_proceed lets the
// restart go ahead, atelier_updater_decline keeps the app running. Until then the install waits.
- (BOOL)updater:(id)updater shouldPostponeRelaunchForUpdate:(id)item untilInvokingBlock:(void (^)(void))installHandler {
    if (relaunch_callback == NULL) {
        return NO;
    }
    pending_install = [installHandler copy];
    relaunch_callback();
    return YES;
}
@end

// Starts Sparkle. False when the bundle has no Sparkle.framework or no feed, as a build from source has not.
bool atelier_updater_start(atelier_relaunch_callback callback) {
    if (controller != nil) {
        return true;
    }
    NSBundle *host = [NSBundle mainBundle];
    if ([host objectForInfoDictionaryKey:@"SUFeedURL"] == nil || [host objectForInfoDictionaryKey:@"SUPublicEDKey"] == nil) {
        return false;
    }
    NSString *path = [[host privateFrameworksPath] stringByAppendingPathComponent:@"Sparkle.framework"];
    NSBundle *sparkle = [NSBundle bundleWithPath:path];
    if (sparkle == nil || ![sparkle load]) {
        return false;
    }
    Class controller_class = NSClassFromString(@"SPUStandardUpdaterController");
    if (controller_class == nil) {
        return false;
    }
    relaunch_callback = callback;
    updater_delegate = [AtelierUpdaterDelegate new];
    SEL init = NSSelectorFromString(@"initWithStartingUpdater:updaterDelegate:userDriverDelegate:");
    controller = ((id (*)(id, SEL, BOOL, id, id))objc_msgSend)([controller_class alloc], init, YES, updater_delegate, nil);
    return controller != nil;
}

// Looks for an update now, and shows Sparkle's own window with the answer.
void atelier_updater_check(void) {
    if (controller == nil) {
        return;
    }
    ((void (*)(id, SEL, id))objc_msgSend)(controller, NSSelectorFromString(@"checkForUpdates:"), nil);
}

// The app agrees to restart: the update installs.
void atelier_updater_proceed(void) {
    void (^install)(void) = pending_install;
    pending_install = nil;
    if (install != nil) {
        install();
    }
}

// The app stays as it is. Sparkle keeps the update and asks again at its next check.
void atelier_updater_decline(void) {
    pending_install = nil;
}
