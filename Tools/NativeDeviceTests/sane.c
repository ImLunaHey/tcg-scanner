/* Deterministic test-only SANE 1 ABI implementation. No hardware access. */
#include <stdatomic.h>
#include <stddef.h>
#include <string.h>
#include <time.h>

typedef struct { const char *name, *vendor, *model, *type; } Device;
typedef struct { int min, max, quant; } Range;
typedef struct {
    const char *name, *title, *description;
    int type, unit, size, cap, constraint_type;
    union { const char **strings; const int *words; const Range *range; } constraint;
} Descriptor;
typedef struct { int format, last_frame, bytes_per_line, pixels_per_line, lines, depth; } Parameters;
static Device device = {"test:cards", "Fixture", "Duplex", "test scanner"};
static const Device *devices[] = {&device, NULL};
static const char *sources[] = {"Flatbed", "ADF Front", "ADF Duplex", NULL};
static const char *modes[] = {"Gray", "Color", NULL};
static const char *backgrounds[] = {"Default", "Black", "White", NULL};
static const char *overscans[] = {"Default", "On", "Off", NULL};
static Range broad = {0, 32767 * 65536, 0};
static Descriptor descriptors[13];
static int numbers[13], page, offset;
static char strings[13][32];
static atomic_int starts, closes, opened, reading, closed_while_reading, cancelled, cancels, scenario;

void fixture_configure(int mode) {
    atomic_store(&scenario, mode);
    atomic_store(&starts, 0); atomic_store(&closes, 0); atomic_store(&opened, 0);
    atomic_store(&reading, 0); atomic_store(&closed_while_reading, 0);
    atomic_store(&cancelled, 0); atomic_store(&cancels, 0);
}
int fixture_counter(int index) {
    switch (index) {
        case 0: return atomic_load(&starts);
        case 1: return atomic_load(&closes);
        case 2: return numbers[3];
        case 3: return atomic_load(&reading);
        case 4: return atomic_load(&closed_while_reading);
        case 5: return atomic_load(&cancels);
        case 6: return atomic_load(&opened);
        default: return -1;
    }
}
int sane_init(int *version, void *authorization) {
    (void)authorization;
    *version = 1 << 24;
    return 0;
}
void sane_exit(void) {}
int sane_get_devices(const Device ***result, int local_only) {
    (void)local_only; *result = devices; return 0;
}
static void configure_options(void) {
    const char *names[] = {"", "source", "mode", "resolution", "tl-x", "tl-y", "br-x", "br-y", "swskip", "swcrop", "swdeskew", "bgcolor", "overscan"};
    memset(numbers, 0, sizeof(numbers)); memset(strings, 0, sizeof(strings));
    for (int i = 0; i < 13; ++i) {
        descriptors[i] = (Descriptor){names[i], names[i], "test option", 1, 0, 4, 5, 1, {.range = &broad}};
    }
    int text_indices[] = {1, 2, 11, 12};
    const char **lists[] = {sources, modes, backgrounds, overscans};
    for (int i = 0; i < 4; ++i) {
        int index = text_indices[i];
        descriptors[index].type = 3; descriptors[index].size = 32;
        descriptors[index].constraint_type = 3; descriptors[index].constraint.strings = lists[i];
        strcpy(strings[index], lists[i][0]);
    }
    numbers[3] = 300; descriptors[3].unit = 4;
    for (int i = 4; i <= 8; ++i) { descriptors[i].type = 2; descriptors[i].unit = i == 8 ? 5 : 3; }
    numbers[6] = 210 * 65536; numbers[7] = 297 * 65536; numbers[8] = 65536;
    descriptors[9].type = descriptors[10].type = 0;
    numbers[9] = numbers[10] = 1;
}
int sane_open(const char *name, void **handle) {
    if (strcmp(name, device.name)) return 4;
    if (atomic_exchange(&opened, 1)) return 3;
    configure_options(); page = offset = 0; *handle = &device; return 0;
}
void sane_close(void *handle) {
    (void)handle;
    if (atomic_load(&reading)) atomic_store(&closed_while_reading, 1);
    atomic_store(&opened, 0); atomic_fetch_add(&closes, 1);
}
const Descriptor *sane_get_option_descriptor(void *handle, int option) {
    (void)handle; return option >= 0 && option < 13 ? &descriptors[option] : NULL;
}
int sane_control_option(void *handle, int option, int action, void *value, int *info) {
    (void)handle;
    if (info) *info = 0;
    if (option == 0 && action == 0) { *(int *)value = 13; return 0; }
    if (option < 1 || option >= 13) return 4;
    if (action == 0) {
        if (descriptors[option].type == 3) memcpy(value, strings[option], 32);
        else *(int *)value = numbers[option];
        return 0;
    }
    if (action != 1) return 1;
    if (descriptors[option].type == 3) memcpy(strings[option], value, 32);
    else numbers[option] = *(int *)value;
    if (option == 1) { numbers[3] = 75; if (info) *info = 2; }
    if (atomic_load(&scenario) == 2 && option == 3 && numbers[3] == 600) {
        numbers[3] = 599; if (info) *info = 1;
    }
    return 0;
}
int sane_get_parameters(void *handle, Parameters *parameters) {
    (void)handle; *parameters = (Parameters){1, 1, 8, 2, 2, 8}; return 0;
}
int sane_start(void *handle) {
    (void)handle;
    atomic_fetch_add(&starts, 1);
    if (page == 2) return 7;
    atomic_store(&cancelled, 0); ++page; offset = 0; return 0;
}
int sane_read(void *handle, unsigned char *data, int maximum, int *length) {
    (void)handle;
    *length = 0;
    if (atomic_load(&scenario) == 1) {
        atomic_store(&reading, 1);
        const struct timespec tick = {0, 1000000};
        while (!atomic_load(&cancelled)) nanosleep(&tick, NULL);
        const struct timespec cleanup = {0, 100000000}; nanosleep(&cleanup, NULL);
        atomic_store(&reading, 0); return 2;
    }
    if (offset == 16) return 5;
    const unsigned char bytes[] = {1,2,3,4,5,6,99,99,7,8,9,10,11,12,99,99};
    *length = 16 - offset < 3 ? 16 - offset : 3;
    if (maximum < *length) *length = maximum;
    memcpy(data, bytes + offset, (size_t)*length); offset += *length; return 0;
}
void sane_cancel(void *handle) {
    (void)handle; atomic_fetch_add(&cancels, 1); atomic_store(&cancelled, 1);
}
