// Own-window screenshot helper. CGWindowListCreateImage is marked
// SDK-unavailable on macOS 15+ but may still exist at runtime; resolve it
// via dlsym. Capturing windows owned by an arbitrary PID requires Screen
// Recording permission on modern macOS, which this machine may not grant.
// Where capture fails, the tool reports that explicitly.
//
// Usage: macos_shot <window_id> <out.png>
#include <CoreGraphics/CoreGraphics.h>
#include <ImageIO/ImageIO.h>
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>

typedef CGImageRef (*CGWindowListCreateImageFn)(CGRect, CGWindowListOption,
                                                CGWindowID, CGWindowImageOption);

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: macos_shot <window_id> <out.png>\n");
        return 64;
    }
    CGWindowListCreateImageFn create =
        (CGWindowListCreateImageFn)dlsym(RTLD_DEFAULT, "CGWindowListCreateImage");
    if (create == NULL) {
        fprintf(stderr, "CGWindowListCreateImage symbol missing at runtime\n");
        return 1;
    }
    CGWindowID id = (CGWindowID)strtoul(argv[1], NULL, 10);
    CGImageRef image = create(
        CGRectNull, kCGWindowListOptionIncludingWindow, id,
        kCGWindowImageNominalResolution);
    if (image == NULL) {
        fprintf(stderr, "CGWindowListCreateImage returned NULL (Screen Recording permission likely required)\n");
        return 1;
    }
    if (CGImageGetWidth(image) == 0 || CGImageGetHeight(image) == 0) {
        fprintf(stderr, "captured image is empty\n");
        return 1;
    }
    CFStringRef path = CFStringCreateWithCString(NULL, argv[2], kCFStringEncodingUTF8);
    CFURLRef url = CFURLCreateWithFileSystemPath(NULL, path, kCFURLPOSIXPathStyle, false);
    CGImageDestinationRef dest = CGImageDestinationCreateWithURL(url, CFSTR("public.png"), 1, NULL);
    CGImageDestinationAddImage(dest, image, NULL);
    if (!CGImageDestinationFinalize(dest)) {
        fprintf(stderr, "image finalize failed\n");
        return 1;
    }
    printf("wrote %s %zux%zu\n", argv[2], CGImageGetWidth(image), CGImageGetHeight(image));
    CFRelease(dest);
    CFRelease(url);
    CFRelease(path);
    CFRelease(image);
    return 0;
}
