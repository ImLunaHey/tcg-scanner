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
typedef struct {int16_t whole;uint16_t fraction;} Fix;
typedef struct {Fix x,y;int32_t width,height;int16_t samples,bits[8],bits_per_pixel;uint16_t planar;int16_t pixel_type;uint16_t compression;} ImageInfo;
typedef struct {uint32_t flags,length;void *pointer;} Memory;
typedef struct {uint16_t compression;uint32_t stride,columns,rows,x,y,written;Memory memory;} Transfer;
typedef struct {void *native;uint16_t message;} Event;
typedef struct {uint16_t count;uint32_t reserved;} Pending;
#pragma pack(pop)
static int opened, source_opened, allocated, locked, scenario, close_dsm_calls;
static uint16_t condition;
static int phase, pages, row, invalid_sequence, enables, ends, resets, disables;
static const uint16_t ids[]={0x1002,0x1007,0x1013,0x0101,0x112b,0x0102,0x1122,0x1118,0x1119,0x0103,1,0x1134,0x1150,0x1151,0x1152,0x1149,0x1003};
static One values[17];
static void reset_options(void) {
    const One initial[]={{6,0},{6,0},{6,0},{4,1},{4,8},{4,0},{4,1},{7,300},{7,300},{4,0},{1,1},{2,0xffffffff},{6,1},{6,1},{6,1},{4,0},{6,1}};
    memcpy(values,initial,sizeof(values));
}
static uint16_t image_operation(uint16_t dat,void *data) {
    if (phase!=6 && phase!=7) {++invalid_sequence;condition=11;return 1;}
    if (dat==0x101) {*(ImageInfo *)data=(ImageInfo){{600,0},{600,0},2,2,3,{8,8,8,0,0,0,0,0},24,0,2,0};return 0;}
    if (dat!=0x103){condition=9;return 1;}
    if (scenario==3){phase=7;return 3;}
    if (scenario==10){phase=7;return 2;}
    if (scenario==4 || (scenario==5 && row>0)){condition=5;return 1;}
    phase=7;Transfer *transfer=data;
    const unsigned char pixels[2][8]={{1,2,3,4,5,6,99,99},{7,8,9,10,11,12,99,99}};
    if (transfer->memory.length<8 || row>=2){condition=10;return 1;}
    memcpy(transfer->memory.pointer,pixels[row],8);
    transfer->compression=0;transfer->stride=8;transfer->columns=2;transfer->rows=1;transfer->x=0;transfer->y=(uint32_t)row;transfer->written=8;
    ++row;return row==2?6:0;
}
void fixture_configure(int mode) {
    opened = source_opened = allocated = locked = close_dsm_calls = 0;
    scenario = mode; condition = 0; phase=pages=row=invalid_sequence=enables=ends=resets=disables=0;reset_options();
}
int fixture_counter(int index) {
    switch(index) {
        case 0: return opened;
        case 1: return source_opened;
        case 2: return allocated;
        case 3: return locked;
        case 4: return close_dsm_calls;
        case 5: return invalid_sequence;
        case 6: return enables;
        case 7: return ends;
        case 8: return resets;
        case 9: return disables;
        case 10: return (int)values[7].value;
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
    if (group==2 && message==1 && source_opened) return image_operation(dat,data);
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
        if (message == 0x401) {if(source_opened)return fail(11);source_opened=1;phase=4;reset_options();return 0;}
        if (message == 0x402) {if(!source_opened || scenario==1)return fail(11);if(phase!=4){++invalid_sequence;return fail(11);}source_opened=0;phase=3;return 0;}
    }
    if (!source_opened) return fail(11);
    if (dat==1 && (message==1 || message==2 || message==8 || message==6)) {
        Capability *capability=data;
        int index=-1;for(int i=0;i<17;++i)if(ids[i]==capability->id)index=i;
        if(index<0)return fail(13);
        if(message==6){
            if(phase!=4){++invalid_sequence;return fail(11);}
            values[index]=*(One *)capability->handle;
            if(scenario==2 && capability->id==0x1118 && values[index].value==600){values[index].value=599;condition=10;return 2;}
            if(capability->id==0x0101)values[7].value=75;
            return 0;
        }
        if(message==1 && (capability->id==0x1118 || capability->id==0x1119)) {
            unsigned char *range=allocate(22);
            uint16_t kind=7; uint32_t values_range[5]={150,600,150,300,300};
            memcpy(range,&kind,2); memcpy(range+2,values_range,20);
            capability->type=6;capability->handle=range;return 0;
        }
        One *one=allocate(sizeof(One));
        *one=message==8?(One){5,capability->id==0x1003?8:10}:values[index];
        capability->type=5;capability->handle=one;return 0;
    }
    if(dat==9 && message==0x502){if(phase!=4){++invalid_sequence;return fail(11);}phase=5;pages=row=0;++enables;return 0;}
    if(dat==9 && message==0x501){if(phase!=5){++invalid_sequence;return fail(11);}phase=4;++disables;return 0;}
    if(dat==2 && message==0x601){Event *event=data;event->message=0;if(phase==5){phase=6;event->message=0x101;}return 4;}
    if(dat==6 && message==1){uint32_t *sizes=data;sizes[0]=8;sizes[1]=65536;sizes[2]=8;return 0;}
    if(dat==5 && message==0x701){
        ++ends;if(phase!=7){++invalid_sequence;return fail(11);}if(scenario==6)return fail(5);
        ++pages;row=0;Pending *pending=data;pending->count=pages<(scenario==7?1:2)?1:0;
        phase=pending->count?6:5;return 0;
    }
    if(dat==5 && message==7){++resets;if(phase!=6){++invalid_sequence;return fail(11);}phase=5;((Pending *)data)->count=0;return 0;}
    return fail(9);
}
