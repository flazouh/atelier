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
// Whether Sparkle may restart the app. Sparkle asks the app once, through the postpone call below, and after that it
// installs on any press of Install and Relaunch, so a no must also stop that second press: this is NO from the moment
// the app is asked until it says yes, or until the update ends.
static BOOL relaunch_allowed = YES;

@interface AtelierUpdaterDelegate : NSObject
@end

@implementation AtelierUpdaterDelegate
// Sparkle has an update ready and wants to restart the app. The app answers: atelier_updater_proceed lets the
// restart go ahead, atelier_updater_decline keeps the app running. Until then the install waits.
- (BOOL)updater:(id)updater shouldPostponeRelaunchForUpdate:(id)item untilInvokingBlock:(void (^)(void))installHandler {
    if (relaunch_callback == NULL) {
        return NO;
    }
    relaunch_allowed = NO;
    pending_install = [installHandler copy];
    relaunch_callback();
    return YES;
}

// Sparkle asks this on every try to install and restart. A no ends the update without installing it.
- (BOOL)updaterShouldRelaunchApplication:(id)updater {
    return relaunch_allowed;
}

// The update ended, installed or not: the next one asks the app again.
- (void)updater:(id)updater didFinishUpdateCycleForUpdateCheck:(NSInteger)updateCheck error:(NSError *)error {
    relaunch_allowed = YES;
    pending_install = nil;
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
    relaunch_allowed = YES;
    if (install != nil) {
        install();
    }
}

// The app stays as it is: the update does not install, even on another press of Install and Relaunch.
void atelier_updater_decline(void) {
    pending_install = nil;
}
