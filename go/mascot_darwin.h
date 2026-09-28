// C bridge between the Go candidate and the AppKit implementation in
// mascot_darwin.m. All objects cross the boundary as opaque pointers; all
// callbacks into Go go through the goUI* exports in platform_darwin.go.
#ifndef MASCOT_DARWIN_H
#define MASCOT_DARWIN_H

#include <stdbool.h>

typedef struct {
    void *window;
    void *input;
    void *response;
    void *status;
    void *send;
    void *cancel;
} MascotComposer;

int  mascot_app_init(void);
void *mascot_create_mascot(const unsigned char *rgba, int src_w, int src_h, double dip);
void mascot_order_front(void *panel);
MascotComposer mascot_create_composer(void *panel, double width, double height,
                                      double input_h, double response_h, double margin);
void mascot_show_composer(MascotComposer composer);
void mascot_hide_composer(MascotComposer composer);
int  mascot_composer_visible(MascotComposer composer);
char *mascot_text_get(void *text_view);
void mascot_text_set(void *text_view, const char *utf8);
void mascot_text_append(void *text_view, const char *utf8);
void mascot_selection_get(void *text_view, long *location, long *length);
void mascot_selection_set(void *text_view, long location, long length);
void mascot_status_set(void *field, const char *utf8);
int  mascot_has_marked(void *text_view);
void mascot_unmark(void *text_view);
int  mascot_register_hotkeys(void);
void mascot_unregister_hotkeys(void);
void mascot_post_ui(void);
void mascot_post_control(void);
void mascot_run(void);
void mascot_terminate(void);
void mascot_teardown(MascotComposer composer, void *panel);

// Go exports consumed by mascot_darwin.m.
void goDispatchUIEvents(void);
void goDispatchControl(void);
void goUIHotkey(int id);
void goUISubmit(void);
void goUIMarked(int has_marked);
void goUIUnmark(void);
void goUISendAction(void);
void goUICancelAction(void);
int  goUIWindowShouldClose(void);
int  goUIShouldInsert(long affected_loc, long affected_len, char *replacement);
void goUIPaint(void);
void goUIPresent(void);

#endif
