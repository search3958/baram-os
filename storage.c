#include "storage.h"
#include "drivers.h"
#include <stdint.h>
#include <stddef.h>

#define ATA_PRIMARY_DATA         0x1F0
#define ATA_PRIMARY_ERR          0x1F1
#define ATA_PRIMARY_SECCOUNT     0x1F2
#define ATA_PRIMARY_LBA_LOW      0x1F3
#define ATA_PRIMARY_LBA_MID      0x1F4
#define ATA_PRIMARY_LBA_HIGH     0x1F5
#define ATA_PRIMARY_DRV_HEAD     0x1F6
#define ATA_PRIMARY_STATUS       0x1F7
#define ATA_PRIMARY_COMMAND      0x1F7

#define ATA_CMD_READ_PIO         0x20
#define ATA_CMD_WRITE_PIO        0x30
#define ATA_CMD_CACHE_FLUSH      0xE7

static int ata_wait_bsy() {
    uint32_t timeout = 1000000;
    while (timeout--) {
        if (!(inb(ATA_PRIMARY_STATUS) & ATA_STATUS_BSY)) return 0;
    }
    return -1;
}

static int ata_wait_drq() {
    uint32_t timeout = 1000000;
    while (timeout--) {
        if (inb(ATA_PRIMARY_STATUS) & ATA_STATUS_DRQ) return 0;
    }
    return -1;
}

void ata_init() {
    // Select drive (LBA mode, Primary Master)
    outb(ATA_PRIMARY_DRV_HEAD, 0xE0);
    // Short delay
    for(int i=0; i<4; i++) inb(ATA_PRIMARY_STATUS);
}

int ata_read_sectors(uint32_t lba, uint32_t count, void *buffer) {
    uint8_t *ptr = (uint8_t *)buffer;
    uint32_t remaining = count;
    uint32_t current_lba = lba;

    while (remaining > 0) {
        uint8_t chunk = (remaining > 255) ? 255 : (uint8_t)remaining;
        uint16_t *buf = (uint16_t *)ptr;

        if (ata_wait_bsy() != 0) return -1;

        outb(ATA_PRIMARY_DRV_HEAD, 0xE0 | ((current_lba >> 24) & 0x0F));
        outb(ATA_PRIMARY_SECCOUNT, chunk);
        outb(ATA_PRIMARY_LBA_LOW, (uint8_t)current_lba);
        outb(ATA_PRIMARY_LBA_MID, (uint8_t)(current_lba >> 8));
        outb(ATA_PRIMARY_LBA_HIGH, (uint8_t)(current_lba >> 16));
        outb(ATA_PRIMARY_COMMAND, ATA_CMD_READ_PIO);

        for (int j = 0; j < chunk; j++) {
            if (ata_wait_bsy() != 0) return -1;
            if (ata_wait_drq() != 0) return -1;

            for (int i = 0; i < 256; i++) {
                uint16_t data;
#ifdef __aarch64__
                data = 0; // No ATA on ARM64 yet
#else
                __asm__ __volatile__("inw %w1, %w0" : "=a"(data) : "Nd"(ATA_PRIMARY_DATA));
#endif
                buf[j * 256 + i] = data;
            }
        }
        
        ptr += chunk * 512;
        remaining -= chunk;
        current_lba += chunk;
    }
    return 0;
}

int ata_write_sectors(uint32_t lba, uint32_t count, const void *buffer) {
    uint8_t *ptr = (uint8_t *)buffer;
    uint32_t remaining = count;
    uint32_t current_lba = lba;

    while (remaining > 0) {
        uint8_t chunk = (remaining > 255) ? 255 : (uint8_t)remaining;
        uint16_t *buf = (uint16_t *)ptr;

        if (ata_wait_bsy() != 0) return -1;

        outb(ATA_PRIMARY_DRV_HEAD, 0xE0 | ((current_lba >> 24) & 0x0F));
        outb(ATA_PRIMARY_SECCOUNT, chunk);
        outb(ATA_PRIMARY_LBA_LOW, (uint8_t)current_lba);
        outb(ATA_PRIMARY_LBA_MID, (uint8_t)(current_lba >> 8));
        outb(ATA_PRIMARY_LBA_HIGH, (uint8_t)(current_lba >> 16));
        outb(ATA_PRIMARY_COMMAND, ATA_CMD_WRITE_PIO);

        for (int j = 0; j < chunk; j++) {
            if (ata_wait_bsy() != 0) return -1;
            if (ata_wait_drq() != 0) return -1;

            for (int i = 0; i < 256; i++) {
                uint16_t data = buf[j * 256 + i];
#ifdef __aarch64__
                (void)data; // No ATA on ARM64 yet
#else
                __asm__ __volatile__("outw %w0, %w1" : : "a"(data), "Nd"(ATA_PRIMARY_DATA));
#endif
            }
        }
        
        ptr += chunk * 512;
        remaining -= chunk;
        current_lba += chunk;
    }

    outb(ATA_PRIMARY_COMMAND, ATA_CMD_CACHE_FLUSH);
    ata_wait_bsy();
    return 0;
}
