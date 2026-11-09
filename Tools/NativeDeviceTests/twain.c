/* Test-only Windows TWAIN 2 ABI fixture. Runs on Unix for native ABI tests.
   No window, scanner or real DSM is opened. */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#pragma pack(push, 2)
typedef struct { uint16_t major, minor, language, country; unsigned char info[34]; } Version;
typedef struct {
    uint32_t id; Version version; uint16_t major, minor; uint32_t groups;
    unsigned char manufacturer[34], family[34], product[34];
} Identity;
typedef uint16_t (*Entry)(Identity *, Identity *, uint32_t, uint16_t, uint16_t, void *);
typedef struct {
    uint32_t size; Entry entry; void *(*allocate)(uint32_t); void (*release)(void *);
    void *(*lock)(void *); void (*unlock)(void *);
} EntryPoints;
typedef struct { uint16_t id, type; void *handle; } Capability;
typedef struct { uint16_t type; uint32_t value; } One;
#pragma pack(pop)
static int opened, source_opened, allocated, locked, scenario, close_dsm_calls;
static uint16_t condition;
void fixture_configure(int mode) {
    opened = source_opened = allocated = locked = close_dsm_calls = 0;
    scenario = mode; condition = 0;
}
int fixture_counter(int index) {
    switch(index) {
        case 0: return opened;
        case 1: return source_opened;
        case 2: return allocated;
        case 3: return locked;
        case 4: return close_dsm_calls;
        default: return -1;
    }
}
static void *allocate(uint32_t size) { ++allocated; return calloc(1,size); }
static void release(void *handle) { --allocated; free(handle); }
static void *lock(void *handle) { ++locked; return handle; }
static void unlock(void *handle) { (void)handle; --locked; }
static uint16_t fail(uint16_t code) {condition=code; return 1;}
uint16_t DSM_Entry(Identity *app, Identity *source, uint32_t group, uint16_t dat, uint16_t message, void *data) {
    (void)source;
    if (group != 1) return fail(9);
    if (dat == 8 && message == 1) { ((uint16_t *)data)[0]=condition; ((uint16_t *)data)[1]=0; condition=0; return 0; }
    if (dat == 4 && message == 0x301) { if(opened)return fail(11); opened=1; app->groups|=0x10000000; return 0; }
    if (!opened) return fail(11);
    if (dat == 4 && message == 0x302) {++close_dsm_calls; if(source_opened)return fail(11);opened=0;return 0;}
    if (dat == 0x403 && message == 1) {
        EntryPoints *entries=data;
        if (entries->size != sizeof(EntryPoints)) return fail(10);
        *entries=(EntryPoints){sizeof(EntryPoints),DSM_Entry,allocate,release,lock,unlock}; return 0;
    }
    if (dat == 3) {
        if (message == 4) {
            Identity *identity=data;memset(identity,0,sizeof(*identity));identity->id=42;
            identity->version.major=2;identity->major=2;identity->minor=5;
            strcpy((char *)identity->manufacturer,"Fixture");strcpy((char *)identity->family,"Test");strcpy((char *)identity->product,"Cards");return 0;
        }
        if (message == 5) return 7;
        if (message == 0x401) {if(source_opened)return fail(11);source_opened=1;return 0;}
        if (message == 0x402) {if(!source_opened || scenario==1)return fail(11);source_opened=0;return 0;}
    }
    if (dat == 1 && source_opened && (message == 2 || message == 8)) {
        Capability *capability=data;
        if (capability->id != 0x1118 && capability->id != 0x1003) return fail(13);
        One *one=allocate(sizeof(One));
        if (message==8) *one=(One){5,capability->id==0x1118?10:8};
        else if (capability->id==0x1118) *one=(One){7,600};
        else *one=(One){6,1};
        capability->type=5;capability->handle=one;return 0;
    }
    return fail(9);
}
