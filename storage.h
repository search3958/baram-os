#ifndef STORAGE_H
#define STORAGE_H

#include <stdint.h>
#include <stddef.h>

#define ATA_SECTOR_SIZE 512

// ATA Status bits
#define ATA_STATUS_BSY  0x80
#define ATA_STATUS_DRDY 0x40
#define ATA_STATUS_DF   0x20
#define ATA_STATUS_DSC  0x10
#define ATA_STATUS_DRQ  0x08
#define ATA_STATUS_CORR 0x04
#define ATA_STATUS_IDX  0x02
#define ATA_STATUS_ERR  0x01

void ata_init();
int ata_read_sectors(uint32_t lba, uint32_t count, void *buffer);
int ata_write_sectors(uint32_t lba, uint32_t count, const void *buffer);

#endif
