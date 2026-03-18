#ifndef FS_H
#define FS_H

#include <stdint.h>
#include <stddef.h>

#define FS_MAGIC 0x42415241 // "BARA"
#define MAX_FILES 64

typedef struct {
    char name[64];
    uint32_t start_lba;
    uint32_t size_bytes;
} fs_entry_t;

typedef struct {
    uint32_t magic;
    uint32_t num_files;
    fs_entry_t entries[MAX_FILES];
} fs_superblock_t;

extern fs_superblock_t g_sb;
void fs_init();
int fs_format();
int fs_write_file(const char *name, const void *data, uint32_t size);
void* fs_read_file(const char *name, uint32_t *out_size);
void fs_list_files();
void fs_get_usage(uint32_t *used_bytes, uint32_t *total_bytes);
#endif

