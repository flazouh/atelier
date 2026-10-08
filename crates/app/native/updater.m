// The Mac updater: Sparkle's engine with the app's own user driver. Sparkle checks for an update, downloads it (as a small
// patch when one fits), verifies its signature and installs it; it shows nothing. Every step it would show is sent to the
// app as one line of JSON, and the app draws it in its own window (src/shell/update_view.rs). The app tells back, with
// atelier_updater_install and atelier_updater_later, what the reader chose.
//
// Sparkle is found at run time, not at link time, so a build from source (which has no Sparkle) runs and says it cannot
// update. The Rust side is src/updater/impls/sparkle_driver.rs.
//
// An update found is downloaded at once: the reader chooses only when to restart. The events, in the order they come:
//   checking {user}, found {version, notes, user}, downloading {fraction}, extracting {fraction}, ready, installing,
//   up_to_date, failed {message}, idle, focus.
#import <AppKit/AppKit.h>
#import <objc/message.h>
#import <objc/runtime.h>
#include <stdbool.h>

typedef void (*atelier_relaunch_callback)(void);
typedef void (*atelier_event_callback)(const char *line);

// SPUUserUpdateChoice, in Sparkle's own order.
enum { ChoiceSkip = 0, ChoiceInstall = 1, ChoiceDismiss = 2 };

static atelier_relaunch_callback relaunch_callback;
static atelier_event_callback event_callback;
static id updater;
static id updater_delegate;
static id user_driver;
static void (^pending_install)(void);
// Sparkle's wait at "ready to install": the reader's choice answers it.
static void (^ready_reply)(NSInteger);
static uint64_t expected_length;
static uint64_t received_length;
static double last_fraction;
// Whether Sparkle may restart the app. Sparkle asks the app once, through the postpone call below, and after that it
// installs on any press of Install and Relaunch, so a no must also stop that second press: this is NO from the moment
// the app is asked until it says yes, or until the update ends.
static BOOL relaunch_allowed = YES;

// Tells the app one event, as a line of JSON.
static void emit(NSDictionary *event) {
    if (event_callback == NULL) {
        return;
    }
    NSData *data = [NSJSONSerialization dataWithJSONObject:event options:0 error:nil];
    if (data == nil) {
        return;
    }
    NSString *line = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    event_callback(line.UTF8String);
}

// Tells the app how far the download is, no more often than each hundredth.
static void emit_download(BOOL force) {
    double fraction = expected_length > 0 ? (double)received_length / (double)expected_length : 0.0;
    if (fraction > 1.0) {
        fraction = 1.0;
    }
    if (force || fraction - last_fraction >= 0.01) {
        last_fraction = fraction;
        emit(@{@"kind": @"downloading", @"fraction": @(fraction)});
    }
}

@interface AtelierUserDriver : NSObject
@end

@implementation AtelierUserDriver
// The reader is asked once whether to check for updates on their own. The bundle already says yes, so this is a yes.
- (void)showUpdatePermissionRequest:(id)request reply:(void (^)(id))reply {
    Class response_class = NSClassFromString(@"SUUpdatePermissionResponse");
    SEL init = NSSelectorFromString(@"initWithAutomaticUpdateChecks:sendSystemProfile:");
    id response = ((id (*)(id, SEL, BOOL, BOOL))objc_msgSend)([response_class alloc], init, YES, NO);
    reply(response);
}

- (void)showUserInitiatedUpdateCheckWithCancellation:(void (^)(void))cancellation {
    emit(@{@"kind": @"checking", @"user": @YES});
}

// An update exists. It downloads at once; the app shows the changelog when it is ready.
- (void)showUpdateFoundWithAppcastItem:(id)item state:(id)state reply:(void (^)(NSInteger))reply {
    if ([[item valueForKey:@"informationOnlyUpdate"] boolValue]) {
        reply(ChoiceDismiss);
        emit(@{@"kind": @"idle"});
        return;
    }
    NSString *version = [item valueForKey:@"displayVersionString"] ?: @"";
    NSString *notes = [item valueForKey:@"itemDescription"];
    BOOL user = [[state valueForKey:@"userInitiated"] boolValue];
    emit(@{@"kind": @"found", @"version": version, @"notes": notes ?: [NSNull null], @"user": @(user)});
    reply(ChoiceInstall);
}

- (void)showUpdateNotFoundWithError:(NSError *)error acknowledgement:(void (^)(void))acknowledgement {
    emit(@{@"kind": @"up_to_date"});
    acknowledgement();
}

- (void)showUpdaterError:(NSError *)error acknowledgement:(void (^)(void))acknowledgement {
    emit(@{@"kind": @"failed", @"message": error.localizedDescription ?: @"the update failed"});
    acknowledgement();
}

- (void)showDownloadInitiatedWithCancellation:(void (^)(void))cancellation {
    expected_length = 0;
    received_length = 0;
    last_fraction = 0;
    emit(@{@"kind": @"downloading", @"fraction": @0});
}

- (void)showDownloadDidReceiveExpectedContentLength:(uint64_t)expectedContentLength {
    expected_length = expectedContentLength;
}

- (void)showDownloadDidReceiveDataOfLength:(uint64_t)length {
    received_length += length;
    emit_download(NO);
}

- (void)showDownloadDidStartExtractingUpdate {
    emit_download(YES);
    emit(@{@"kind": @"extracting", @"fraction": @0});
}

- (void)showExtractionReceivedProgress:(double)progress {
    emit(@{@"kind": @"extracting", @"fraction": @(progress)});
}

// The update is downloaded and unpacked. It waits here for the reader: atelier_updater_install or atelier_updater_later.
- (void)showReadyToInstallAndRelaunch:(void (^)(NSInteger))reply {
    ready_reply = [reply copy];
    emit(@{@"kind": @"ready"});
}

- (void)showInstallingUpdateWithApplicationTerminated:(BOOL)applicationTerminated retryTerminatingApplication:(void (^)(void))retryTerminatingApplication {
    emit(@{@"kind": @"installing"});
}

- (void)showUpdateInstalledAndRelaunched:(BOOL)relaunched acknowledgement:(void (^)(void))acknowledgement {
    acknowledgement();
}

- (void)dismissUpdateInstallation {
    ready_reply = nil;
    emit(@{@"kind": @"idle"});
}

- (void)showUpdateInFocus {
    emit(@{@"kind": @"focus"});
}
@end

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
bool atelier_updater_start(atelier_relaunch_callback relaunch, atelier_event_callback event) {
    if (updater != nil) {
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
    Class updater_class = NSClassFromString(@"SPUUpdater");
    if (updater_class == nil) {
        return false;
    }
    relaunch_callback = relaunch;
    event_callback = event;
    // The driver is Sparkle's user driver by what it answers; the protocol is added so Sparkle's own checks see it.
    Protocol *protocol = objc_getProtocol("SPUUserDriver");
    if (protocol != NULL) {
        class_addProtocol([AtelierUserDriver class], protocol);
    }
    user_driver = [AtelierUserDriver new];
    updater_delegate = [AtelierUpdaterDelegate new];
    SEL init = NSSelectorFromString(@"initWithHostBundle:applicationBundle:userDriver:delegate:");
    updater = ((id (*)(id, SEL, id, id, id, id))objc_msgSend)([updater_class alloc], init, host, host, user_driver, updater_delegate);
    if (updater == nil) {
        return false;
    }
    NSError *error = nil;
    BOOL started = ((BOOL (*)(id, SEL, NSError **))objc_msgSend)(updater, NSSelectorFromString(@"startUpdater:"), &error);
    if (!started) {
        updater = nil;
        return false;
    }
    return true;
}

// Looks for an update now. `asked` is whether the reader asked; the answer comes as events.
void atelier_updater_check(bool asked) {
    if (updater == nil) {
        return;
    }
    SEL look = asked ? NSSelectorFromString(@"checkForUpdates") : NSSelectorFromString(@"checkForUpdatesInBackground");
    ((void (*)(id, SEL))objc_msgSend)(updater, look);
}

// The reader chose to restart: the downloaded update installs.
void atelier_updater_install(void) {
    void (^reply)(NSInteger) = ready_reply;
    ready_reply = nil;
    if (reply != nil) {
        reply(ChoiceInstall);
    }
}

// The reader chose to wait: the update installs when the app quits.
void atelier_updater_later(void) {
    void (^reply)(NSInteger) = ready_reply;
    ready_reply = nil;
    if (reply != nil) {
        reply(ChoiceDismiss);
    }
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
