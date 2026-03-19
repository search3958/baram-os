#include "fs.h"
#include "storage.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

extern void set_w1_global(const char *key, const char *val);

#define SUPERBLOCK_LBA 0
#define SUPERBLOCK_SECTORS 16
#define DATA_START_LBA 16

fs_superblock_t g_sb;

void fs_init() {
    // Read superblock from disk
    static uint8_t buf[SUPERBLOCK_SECTORS * 512];
    if (ata_read_sectors(SUPERBLOCK_LBA, SUPERBLOCK_SECTORS, buf) != 0) {
        g_sb.magic = 0;
        g_sb.num_files = 0;
        set_w1_global("--warpSystemLog", "FS:Init ATA Read Fail");
        return;
    }
    memcpy(&g_sb, buf, sizeof(fs_superblock_t));

    if (g_sb.magic != FS_MAGIC) {
        g_sb.num_files = 0;
        set_w1_global("--warpSystemLog", "FS:Magic Mismatch / Not Formatted");
    } else {
        char log[64] = "FS:Init OK. Files: ";
        extern char *append_int(char *p, int v);
        char tmp[16];
        append_int(tmp, (int)g_sb.num_files);
        strlcat(log, tmp, 63);
        set_w1_global("--warpSystemLog", log);
    }
}

int fs_format() {
    set_w1_global("--warpSystemLog", "FS:Formatting...");
    memset(&g_sb, 0, sizeof(fs_superblock_t));
    g_sb.magic = FS_MAGIC;
    g_sb.num_files = 0;

    static uint8_t buf[SUPERBLOCK_SECTORS * 512];
    memset(buf, 0, sizeof(buf));
    memcpy(buf, &g_sb, sizeof(fs_superblock_t));

    if (ata_write_sectors(SUPERBLOCK_LBA, SUPERBLOCK_SECTORS, buf) != 0) {
        set_w1_global("--warpSystemLog", "FS:Format Write Fail");
        return -1;
    }
    set_w1_global("--warpSystemLog", "FS:Format OK");
    return 0;
}

int fs_write_file(const char *name, const void *data, uint32_t size) {
    if (g_sb.num_files >= MAX_FILES) return -1;

    char log[128] = "FS:Write:";
    strlcat(log, name, 127);
    set_w1_global("--warpSystemLog", log);

    // Find last file's end LBA
    uint32_t next_lba = DATA_START_LBA;
    if (g_sb.num_files > 0) {
        fs_entry_t *last = &g_sb.entries[g_sb.num_files - 1];
        uint32_t last_sectors = (last->size_bytes + 511) / 512;
        next_lba = last->start_lba + last_sectors;
    }

    // Write file data
    
    // We can only write in 255 sector chunks with ATA PIO LBA28
    uint32_t remaining_size = size;
    uint32_t current_lba = next_lba;
    const uint8_t *data_ptr = (const uint8_t *)data;

    while (remaining_size > 0) {
        uint32_t chunk_size = (remaining_size > 255 * 512) ? 255 * 512 : remaining_size;
        uint8_t sectors = (chunk_size + 511) / 512;
        
        // Handle alignment: if data is not sector-aligned, we need a temp buffer for the last sector
        if (chunk_size % 512 != 0 && remaining_size == chunk_size) {
            uint8_t temp[512];
            memset(temp, 0, 512);
            uint32_t full_sectors = chunk_size / 512;
            if (full_sectors > 0) {
                ata_write_sectors(current_lba, full_sectors, data_ptr);
            }
            memcpy(temp, data_ptr + full_sectors * 512, chunk_size % 512);
            ata_write_sectors(current_lba + full_sectors, 1, temp);
        } else {
            ata_write_sectors(current_lba, sectors, data_ptr);
        }

        remaining_size -= chunk_size;
        current_lba += sectors;
        data_ptr += chunk_size;
    }

    // Update superblock entry
    fs_entry_t *new_entry = &g_sb.entries[g_sb.num_files];
    strncpy(new_entry->name, name, 63);
    new_entry->name[63] = '\0';
    new_entry->start_lba = next_lba;
    new_entry->size_bytes = size;
    g_sb.num_files++;

    // Write superblock back
    uint8_t sb_buf[SUPERBLOCK_SECTORS * 512];
    memset(sb_buf, 0, sizeof(sb_buf));
    memcpy(sb_buf, &g_sb, sizeof(fs_superblock_t));
    ata_write_sectors(SUPERBLOCK_LBA, SUPERBLOCK_SECTORS, sb_buf);

    return 0;
}

// fs.c の修正
void* fs_read_file(const char *name, uint32_t *out_size) {
    for (uint32_t i = 0; i < g_sb.num_files; i++) {
        if (strcmp(g_sb.entries[i].name, name) == 0) {
            uint32_t size = g_sb.entries[i].size_bytes;
            uint32_t sectors = (size + 511) / 512;
            
            // セクタ単位でメモリ確保（+1は安全のためのnull終端用）
            char *data = (char *)malloc(sectors * 512 + 1);
            if (!data) {
                set_w1_global("--warpSystemLog", "FS:MallocFail");
                return NULL;
            }

            // 大きなファイルの場合は分割読み込み
            uint32_t remaining_sectors = sectors;
            uint32_t current_lba = g_sb.entries[i].start_lba;
            char *write_ptr = data;
            
            while (remaining_sectors > 0) {
                uint32_t chunk_sectors = (remaining_sectors > 255) ? 255 : remaining_sectors;
                
                if (ata_read_sectors(current_lba, chunk_sectors, write_ptr) != 0) {
                    free(data);
                    set_w1_global("--warpSystemLog", "FS:ReadFail");
                    return NULL;
                }
                
                remaining_sectors -= chunk_sectors;
                current_lba += chunk_sectors;
                write_ptr += chunk_sectors * 512;
            }

            data[size] = '\0'; // null終端を保証
            if (out_size) *out_size = size;
            
            // デバッグログ
            char log[128] = "FS:Read:";
            strlcat(log, name, 127);
            strlcat(log, " ", 127);
            char tmp[16];
            extern char *append_int(char *p, int v);
            append_int(tmp, (int)size);
            strlcat(log, tmp, 127);
            strlcat(log, "B", 127);
            set_w1_global("--warpSystemLog", log);
            
            return data;
        }
    }
    
    char log[128] = "FS:NotFound:";
    strlcat(log, name, 127);
    set_w1_global("--warpSystemLog", log);
    return NULL;
}


void fs_get_usage(uint32_t *used_bytes, uint32_t *total_bytes) {
    uint32_t used = 0;
    for (uint32_t i = 0; i < g_sb.num_files; i++) {
        used += g_sb.entries[i].size_bytes;
    }
    if (used_bytes) *used_bytes = used;
    // 64MB as set in bld.sh
    if (total_bytes) *total_bytes = 64 * 1024 * 1024;
}

void fs_list_files() {
    char buf[512] = "Files: ";
    if (g_sb.magic != FS_MAGIC) {
        set_w1_global("--warpSystemLog", "FS Not Formatted");
        return;
    }
    for (uint32_t i = 0; i < g_sb.num_files; i++) {
        if (i > 0) strlcat(buf, ", ", 511);
        strlcat(buf, g_sb.entries[i].name, 511);
    }
    set_w1_global("--warpSystemLog", buf);
}