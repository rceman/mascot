// AppKit implementation for the Go macOS candidate. Everything Cocoa-facing
// lives here so the Go side stays a thin control/orchestration layer over the
// shared provider/control/framing code.
#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>
#include <stdlib.h>
#include <string.h>
#include "mascot_darwin.h"

// Forward declarations for the perch anchor used inside MascotView.
static void perch_mascot(void);
static NSPanel *gMascotPanel;
static NSWindow *gComposerWindow;

// ---------------------------------------------------------------- mascot view

@interface MascotView : NSView {
@public
    NSImage *image;
    const unsigned char *mask;
    int maskW;
    int maskH;
}
@end

@implementation MascotView

- (BOOL)isOpaque { return NO; }

- (NSView *)hitTest:(NSPoint)point {
    // point is in window/content coordinates; the view fills the content view.
    NSPoint local = [self convertPoint:point fromView:nil];
    NSRect bounds = self.bounds;
    if (bounds.size.width <= 0 || bounds.size.height <= 0 || mask == NULL) {
        return nil;
    }
    int sx = (int)(local.x / bounds.size.width * maskW);
    int sy = (int)((bounds.size.height - local.y) / bounds.size.height * maskH);
    if (local.x < 0 || local.y < 0 || sx >= maskW || sy >= maskH) {
        return nil;
    }
    if (mask[(size_t)(sy * maskW + sx) * 4 + 3] > 0) {
        return self;
    }
    return nil;
}

- (void)mouseDown:(NSEvent *)event {
    NSWindow *window = self.window;
    if (gComposerWindow != nil && gComposerWindow.visible) {
        // Anchored drag: a mascot drag while perched moves the composer by
        // the same delta; the composer move re-perches the mascot.
        NSRect cStart = gComposerWindow.frame;
        NSPoint down = event.locationInWindow;
        NSRect wStart = window.frame;
        NSPoint start = NSMakePoint(wStart.origin.x + down.x,
                                    wStart.origin.y + down.y);
        while (YES) {
            NSEvent *e =
                [window nextEventMatchingMask:NSEventMaskLeftMouseDragged |
                                              NSEventMaskLeftMouseUp
                                    untilDate:[NSDate distantFuture]
                                       inMode:NSEventTrackingRunLoopMode
                                      dequeue:YES];
            if (e == nil || e.type == NSEventTypeLeftMouseUp) {
                break;
            }
            NSPoint local = e.locationInWindow;
            NSRect cur = window.frame;
            NSPoint now = NSMakePoint(cur.origin.x + local.x,
                                      cur.origin.y + local.y);
            CGFloat dx = now.x - start.x;
            CGFloat dy = now.y - start.y;
            [gComposerWindow setFrameOrigin:NSMakePoint(cStart.origin.x + dx,
                                                      cStart.origin.y + dy)];
            perch_mascot();
        }
        return;
    }
    [window performWindowDragWithEvent:event];
}

- (BOOL)acceptsFirstMouse:(NSEvent *)event {
    (void)event;
    return YES;
}

- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    goUIPresent();
    [image drawInRect:self.bounds];
}

@end

// ------------------------------------------------------------- text view glue

@interface MascotInputTextView : NSTextView
@end

@implementation MascotInputTextView

- (void)keyDown:(NSEvent *)event {
    // macOS convention: Cmd+Return submits, plain Return inserts a newline.
    BOOL submit = event.keyCode == 36 &&
        (event.modifierFlags & NSEventModifierFlagCommand) != 0;
    if (submit && !self.hasMarkedText) {
        goUISubmit();
        return;
    }
    [super keyDown:event];
}

- (void)drawRect:(NSRect)dirtyRect {
    goUIPaint();
    [super drawRect:dirtyRect];
}

- (void)setMarkedText:(id)string
        selectedRange:(NSRange)selectedRange
     replacementRange:(NSRange)replacementRange {
    [super setMarkedText:string
           selectedRange:selectedRange
        replacementRange:replacementRange];
    goUIMarked(self.hasMarkedText ? 1 : 0);
}

- (void)insertText:(id)string replacementRange:(NSRange)replacementRange {
    [super insertText:string replacementRange:replacementRange];
    goUIMarked(self.hasMarkedText ? 1 : 0);
}

- (void)unmarkText {
    [super unmarkText];
    goUIUnmark();
}

@end

@interface MascotResponseTextView : NSTextView
@end

@implementation MascotResponseTextView

- (void)drawRect:(NSRect)dirtyRect {
    goUIPaint();
    [super drawRect:dirtyRect];
}

@end

// --------------------------------------------------------- composer delegate

@interface MascotComposerDelegate : NSObject <NSWindowDelegate, NSTextViewDelegate>
@property(nonatomic, assign) NSTextView *inputView;
@end

@implementation MascotComposerDelegate

- (BOOL)windowShouldClose:(NSWindow *)sender {
    (void)sender;
    return goUIWindowShouldClose() ? YES : NO;
}

- (void)windowDidMove:(NSNotification *)notification {
    (void)notification;
    perch_mascot();
}

- (void)windowDidResize:(NSNotification *)notification {
    (void)notification;
    perch_mascot();
}

- (BOOL)textView:(NSTextView *)textView
    shouldChangeTextInRange:(NSRange)affectedCharRange
          replacementString:(NSString *)replacementString {
    (void)textView;
    const char *repl = replacementString.UTF8String ?: "";
    return goUIShouldInsert((long)affectedCharRange.location,
                            (long)affectedCharRange.length, (char *)repl) ? YES : NO;
}

- (void)sendAction:(id)sender {
    (void)sender;
    goUISendAction();
}

- (void)cancelAction:(id)sender {
    (void)sender;
    goUICancelAction();
}

@end

// ------------------------------------------------------------------- globals

static MascotComposerDelegate *gDelegate;
static NSTextView *gInputView;
static EventHotKeyRef gToggleKey;
static EventHotKeyRef gCancelKey;
static EventHandlerRef gHotkeyHandler;

// Overlap in points so the mascot appears seated on the composer top edge
// rather than floating above it.
static const CGFloat kPerchOverlap = 6.0;

// The mascot perches centered on the composer top edge while the composer is
// visible. Composer moves/resizes re-anchor it through this helper.
static void perch_mascot(void) {
    if (gMascotPanel == nil || gComposerWindow == nil || !gComposerWindow.visible) {
        return;
    }
    NSRect cf = gComposerWindow.frame;
    NSRect mf = gMascotPanel.frame;
    [gMascotPanel setFrameOrigin:NSMakePoint(
        cf.origin.x + (cf.size.width - mf.size.width) * 0.5,
        NSMaxY(cf) - kPerchOverlap)];
}

static void pump_ui(void *ctx) {
    (void)ctx;
    goDispatchUIEvents();
}

static void pump_control(void *ctx) {
    (void)ctx;
    goDispatchControl();
}

void mascot_post_ui(void) {
    dispatch_async_f(dispatch_get_main_queue(), NULL, pump_ui);
}

void mascot_post_control(void) {
    dispatch_async_f(dispatch_get_main_queue(), NULL, pump_control);
}

int mascot_app_init(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        // MASCOT_REGULAR_APP=1 runs as a regular app for automation
        // experiments (accessory apps cannot be programmatically activated
        // on macOS 26). Shipping mode is accessory.
        const char *regular = getenv("MASCOT_REGULAR_APP");
        [NSApp setActivationPolicy:(regular != NULL && regular[0] != '\0'
            ? NSApplicationActivationPolicyRegular
            : NSApplicationActivationPolicyAccessory)];
    }
    return 1;
}

static NSImage *image_from_rgba(const unsigned char *rgba, int w, int h) {
    NSBitmapImageRep *rep = [[NSBitmapImageRep alloc]
        initWithBitmapDataPlanes:NULL
                      pixelsWide:w
                      pixelsHigh:h
                   bitsPerSample:8
                 samplesPerPixel:4
                        hasAlpha:YES
                        isPlanar:NO
                  colorSpaceName:NSCalibratedRGBColorSpace
                     bytesPerRow:(NSUInteger)w * 4
                    bitsPerPixel:32];
    if (rep == nil || rep.bitmapData == NULL) {
        return nil;
    }
    memcpy(rep.bitmapData, rgba, (size_t)w * (size_t)h * 4);
    NSImage *image = [[NSImage alloc] initWithSize:NSMakeSize(64, 64)];
    [image addRepresentation:rep];
    return image;
}

void *mascot_create_mascot(const unsigned char *rgba, int src_w, int src_h, double dip) {
    NSImage *image = image_from_rgba(rgba, src_w, src_h);
    if (image == nil) {
        return NULL;
    }
    NSPoint mouse = [NSEvent mouseLocation];
    CGFloat x = mouse.x + 16;
    CGFloat y = mouse.y - 16 - dip;
    NSScreen *screen = [NSScreen mainScreen];
    if (screen != nil) {
        NSRect visible = screen.visibleFrame;
        x = MIN(MAX(x, NSMinX(visible)), MAX(NSMinX(visible), NSMaxX(visible) - dip));
        y = MIN(MAX(y, NSMinY(visible)), MAX(NSMinY(visible), NSMaxY(visible) - dip));
    }
    NSPanel *panel = [[NSPanel alloc]
        initWithContentRect:NSMakeRect(x, y, dip, dip)
                  styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                    backing:NSBackingStoreBuffered
                      defer:NO];
    panel.opaque = NO;
    panel.backgroundColor = [NSColor clearColor];
    panel.hasShadow = NO;
    panel.level = NSFloatingWindowLevel;
    panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces |
        NSWindowCollectionBehaviorStationary |
        NSWindowCollectionBehaviorIgnoresCycle |
        NSWindowCollectionBehaviorFullScreenAuxiliary;
    panel.releasedWhenClosed = NO;
    panel.hidesOnDeactivate = NO;

    unsigned char *maskCopy = malloc((size_t)src_w * (size_t)src_h * 4);
    memcpy(maskCopy, rgba, (size_t)src_w * (size_t)src_h * 4);
    MascotView *view = [[MascotView alloc] initWithFrame:NSMakeRect(0, 0, dip, dip)];
    view->image = image;
    view->mask = maskCopy;
    view->maskW = src_w;
    view->maskH = src_h;
    panel.contentView = view;
    gMascotPanel = panel;
    return (__bridge_retained void *)panel;
}

void mascot_order_front(void *panel) {
    [(__bridge NSPanel *)panel orderFrontRegardless];
}

static NSTextView *make_text_view(NSRect frame, BOOL editable) {
    NSTextView *tv;
    if (editable) {
        tv = [[MascotInputTextView alloc] initWithFrame:NSMakeRect(0, 0, frame.size.width, frame.size.height)];
    } else {
        tv = [[MascotResponseTextView alloc] initWithFrame:NSMakeRect(0, 0, frame.size.width, frame.size.height)];
    }
    tv.editable = editable;
    tv.selectable = YES;
    tv.richText = NO;
    tv.importsGraphics = NO;
    tv.usesRuler = NO;
    tv.usesFontPanel = NO;
    tv.font = [NSFont systemFontOfSize:16];
    tv.allowsUndo = editable;
    tv.automaticQuoteSubstitutionEnabled = NO;
    tv.automaticDashSubstitutionEnabled = NO;
    tv.automaticTextReplacementEnabled = NO;
    tv.automaticSpellingCorrectionEnabled = NO;
    tv.automaticLinkDetectionEnabled = NO;
    tv.automaticDataDetectionEnabled = NO;
    tv.smartInsertDeleteEnabled = NO;
    return tv;
}

static NSScrollView *wrap_scroll(NSTextView *tv, NSRect frame) {
    NSScrollView *scroll = [[NSScrollView alloc] initWithFrame:frame];
    scroll.hasVerticalScroller = YES;
    scroll.autohidesScrollers = YES;
    scroll.borderType = NSBezelBorder;
    scroll.documentView = tv;
    return scroll;
}

MascotComposer mascot_create_composer(void *panel, double width, double height,
                                      double input_h, double response_h, double margin) {
    MascotComposer out = {0};
    NSPoint origin = NSMakePoint(0, 0);
    if (panel != NULL) {
        NSRect mrect = [(__bridge NSPanel *)panel frame];
        origin.x = NSMaxX(mrect) + 8;
        origin.y = NSMaxY(mrect) - height;
    }
    NSScreen *screen = [NSScreen mainScreen];
    if (screen != nil) {
        NSRect visible = screen.visibleFrame;
        origin.x = MIN(MAX(origin.x, NSMinX(visible)), MAX(NSMinX(visible), NSMaxX(visible) - width));
        origin.y = MIN(MAX(origin.y, NSMinY(visible)), MAX(NSMinY(visible), NSMaxY(visible) - height));
    }
    NSWindow *window = [[NSWindow alloc]
        initWithContentRect:NSMakeRect(origin.x, origin.y, width, height)
                  styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                            NSWindowStyleMaskMiniaturizable
                    backing:NSBackingStoreBuffered
                      defer:NO];
    window.title = @"mascot";
    window.releasedWhenClosed = NO;

    gDelegate = [[MascotComposerDelegate alloc] init];
    window.delegate = gDelegate;
    gComposerWindow = window;

    NSView *content = window.contentView;
    double input_top = height - margin - input_h;
    double response_top = input_top - 12 - response_h;
    double status_y = response_top - 12 - 28;

    NSTextView *input = make_text_view(
        NSMakeRect(0, 0, width - 2 * margin, input_h), YES);
    input.delegate = gDelegate;
    gDelegate.inputView = input;
    gInputView = input;

    NSTextView *response = make_text_view(
        NSMakeRect(0, 0, width - 2 * margin, response_h), NO);

    [content addSubview:wrap_scroll(input, NSMakeRect(margin, input_top,
        width - 2 * margin, input_h))];
    [content addSubview:wrap_scroll(response, NSMakeRect(margin, response_top,
        width - 2 * margin, response_h))];

    NSTextField *status = [[NSTextField alloc]
        initWithFrame:NSMakeRect(margin, status_y, 416, 28)];
    status.stringValue = @"idle";
    status.editable = NO;
    status.selectable = NO;
    status.bordered = NO;
    status.drawsBackground = NO;
    status.font = [NSFont systemFontOfSize:16];
    [content addSubview:status];

    NSButton *send = [[NSButton alloc]
        initWithFrame:NSMakeRect(width - margin - 80 - 12 - 100, status_y, 80, 28)];
    send.title = @"Send";
    send.bezelStyle = NSBezelStyleRounded;
    send.target = gDelegate;
    send.action = @selector(sendAction:);
    [content addSubview:send];

    NSButton *cancel = [[NSButton alloc]
        initWithFrame:NSMakeRect(width - margin - 100, status_y, 100, 28)];
    cancel.title = @"Cancel";
    cancel.bezelStyle = NSBezelStyleRounded;
    cancel.target = gDelegate;
    cancel.action = @selector(cancelAction:);
    [content addSubview:cancel];

    out.window = (__bridge_retained void *)window;
    out.input = (__bridge_retained void *)input;
    out.response = (__bridge_retained void *)response;
    out.status = (__bridge_retained void *)status;
    out.send = (__bridge_retained void *)send;
    out.cancel = (__bridge_retained void *)cancel;
    return out;
}

void mascot_show_composer(MascotComposer c) {
    NSWindow *window = (__bridge NSWindow *)c.window;
    NSTextView *input = (__bridge NSTextView *)c.input;
    [NSApp activateIgnoringOtherApps:YES];
    [window makeKeyAndOrderFront:nil];
    [window makeFirstResponder:input];
    perch_mascot();
}

void mascot_hide_composer(MascotComposer c) {
    [(__bridge NSWindow *)c.window orderOut:nil];
}

int mascot_composer_visible(MascotComposer c) {
    return [(__bridge NSWindow *)c.window isVisible] ? 1 : 0;
}

char *mascot_text_get(void *text_view) {
    NSString *s = [(__bridge NSTextView *)text_view string];
    NSString *norm = [[s stringByReplacingOccurrencesOfString:@"\r\n"
                                                  withString:@"\n"]
        stringByReplacingOccurrencesOfString:@"\r" withString:@"\n"];
    return strdup(norm.UTF8String);
}

void mascot_text_set(void *text_view, const char *utf8) {
    NSString *s = [[NSString alloc] initWithUTF8String:utf8 ?: ""];
    [(__bridge NSTextView *)text_view setString:s];
}

void mascot_text_append(void *text_view, const char *utf8) {
    NSTextView *tv = (__bridge NSTextView *)text_view;
    NSString *s = [[NSString alloc] initWithUTF8String:utf8 ?: ""];
    [tv.textStorage.mutableString appendString:s];
    NSUInteger len = tv.textStorage.mutableString.length;
    [tv scrollRangeToVisible:NSMakeRange(len > 0 ? len - 1 : 0, 0)];
}

void mascot_selection_get(void *text_view, long *location, long *length) {
    NSRange range = [(__bridge NSTextView *)text_view selectedRange];
    if (location) *location = (long)range.location;
    if (length) *length = (long)range.length;
}

void mascot_selection_set(void *text_view, long location, long length) {
    [(__bridge NSTextView *)text_view
        setSelectedRange:NSMakeRange((NSUInteger)(location < 0 ? 0 : location),
                                     (NSUInteger)(length < 0 ? 0 : length))];
}

void mascot_status_set(void *field, const char *utf8) {
    NSString *s = [[NSString alloc] initWithUTF8String:utf8 ?: ""];
    [(__bridge NSTextField *)field setStringValue:s];
}

int mascot_has_marked(void *text_view) {
    return [(__bridge NSTextView *)text_view hasMarkedText] ? 1 : 0;
}

void mascot_unmark(void *text_view) {
    [(__bridge NSTextView *)text_view unmarkText];
}

// ------------------------------------------------------------------ hotkeys

static OSStatus hotkey_handler(EventHandlerCallRef next, EventRef event, void *user) {
    (void)next;
    (void)user;
    EventHotKeyID hkid;
    GetEventParameter(event, kEventParamDirectObject, typeEventHotKeyID, NULL,
                      sizeof(hkid), NULL, &hkid);
    goUIHotkey((int)hkid.id);
    return noErr;
}

int mascot_register_hotkeys(void) {
    EventTypeSpec types[] = {{kEventClassKeyboard, kEventHotKeyPressed}};
    if (InstallEventHandler(GetApplicationEventTarget(), hotkey_handler, 1,
                            types, NULL, &gHotkeyHandler) != noErr) {
        return 0;
    }
    EventHotKeyID toggle = {'MSCT', 1};
    EventHotKeyID cancel = {'MSCT', 2};
    if (RegisterEventHotKey(49, controlKey | optionKey, toggle,
                            GetApplicationEventTarget(), 0, &gToggleKey) != noErr) {
        return 0;
    }
    if (RegisterEventHotKey(53, controlKey | optionKey, cancel,
                            GetApplicationEventTarget(), 0, &gCancelKey) != noErr) {
        return 0;
    }
    return 1;
}

void mascot_unregister_hotkeys(void) {
    if (gToggleKey != NULL) { UnregisterEventHotKey(gToggleKey); gToggleKey = NULL; }
    if (gCancelKey != NULL) { UnregisterEventHotKey(gCancelKey); gCancelKey = NULL; }
    if (gHotkeyHandler != NULL) { RemoveEventHandler(gHotkeyHandler); gHotkeyHandler = NULL; }
}

// -------------------------------------------------------------- IME helpers
//
// Input-method source selection requires a genuine user gesture on
// macOS 26, so composition is emulated through the same NSTextInputClient
// entry points the IME calls (setMarkedText:/insertText:/unmarkText).

int mascot_ime_select(const char *utf8) {
    @autoreleasepool {
        NSString *identifier = [[NSString alloc] initWithUTF8String:utf8 ?: ""];
        NSTextInputContext *ctx = gInputView.inputContext;
        if (ctx != nil) {
            ctx.selectedKeyboardInputSource = identifier;
        }
        CFArrayRef list = TISCreateInputSourceList(NULL, true);
        if (list == NULL) return -1;
        OSStatus status = paramErr;
        for (CFIndex i = 0; i < CFArrayGetCount(list); i++) {
            TISInputSourceRef source =
                (TISInputSourceRef)CFArrayGetValueAtIndex(list, i);
            CFStringRef sid = (CFStringRef)TISGetInputSourceProperty(
                source, kTISPropertyInputSourceID);
            if (sid != NULL && CFStringCompare(sid, (__bridge CFStringRef)identifier, 0) == 0) {
                status = TISSelectInputSource(source);
                break;
            }
        }
        CFRelease(list);
        return (int)status;
    }
}

void mascot_ime_mark(const char *utf8) {
    @autoreleasepool {
        if (gInputView == nil) return;
        NSString *s = [[NSString alloc] initWithUTF8String:utf8 ?: ""];
        NSRange sel = gInputView.selectedRange;
        [gInputView setMarkedText:s
                    selectedRange:NSMakeRange(sel.location + sel.length, 0)
                 replacementRange:NSMakeRange(NSNotFound, 0)];
    }
}

void mascot_ime_insert(const char *utf8) {
    @autoreleasepool {
        if (gInputView == nil) return;
        NSString *s = [[NSString alloc] initWithUTF8String:utf8 ?: ""];
        [gInputView insertText:s replacementRange:NSMakeRange(NSNotFound, 0)];
    }
}

void mascot_ime_discard(void) {
    @autoreleasepool {
        if (gInputView == nil) return;
        NSRange marked = gInputView.markedRange;
        [gInputView setMarkedText:@""
                    selectedRange:NSMakeRange(marked.location, 0)
                 replacementRange:marked];
        [gInputView unmarkText];
    }
}

int mascot_focus_info(void) {
    @autoreleasepool {
        if (gComposerWindow == nil) return 0;
        int bits = 0;
        if (gComposerWindow.keyWindow) bits |= 1;
        if (gComposerWindow.mainWindow) bits |= 2;
        NSResponder *fr = gComposerWindow.firstResponder;
        if (fr == gInputView) bits |= 4;
        return bits;
    }
}

void mascot_run(void) {
    [NSApp run];
}

void mascot_terminate(void) {
    [NSApp terminate:nil];
}

void mascot_teardown(MascotComposer c, void *panel) {
    if (c.window != NULL) {
        NSWindow *window = (__bridge_transfer NSWindow *)c.window;
        [window orderOut:nil];
        window.delegate = nil;
    }
    if (c.input != NULL) { (void)(__bridge_transfer NSTextView *)c.input; }
    if (c.response != NULL) { (void)(__bridge_transfer NSTextView *)c.response; }
    if (c.status != NULL) { (void)(__bridge_transfer NSTextField *)c.status; }
    if (c.send != NULL) { (void)(__bridge_transfer NSButton *)c.send; }
    if (c.cancel != NULL) { (void)(__bridge_transfer NSButton *)c.cancel; }
    if (panel != NULL) {
        NSPanel *p = (__bridge_transfer NSPanel *)panel;
        [p orderOut:nil];
    }
    gDelegate = nil;
    gInputView = nil;
    gMascotPanel = nil;
    gComposerWindow = nil;
}
