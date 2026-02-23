#ifndef IO_H
#define IO_H

#include <stdint.h>

/* ポートから1バイト読み込む */
static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    __asm__ __volatile__ ("inb %1, %0" : "=a"(ret) : "dN"(port));
    return ret;
}

/* ポートに1バイト書き込む */
static inline void outb(uint16_t port, uint8_t val) {
    __asm__ __volatile__ ("outb %0, %1" : : "a"(val), "dN"(port));
}

#endif /* IO_H */
