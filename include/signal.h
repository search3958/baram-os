#ifndef OS_SIGNAL_H
#define OS_SIGNAL_H

#ifdef __cplusplus
extern "C" {
#endif

typedef int sig_atomic_t;
typedef unsigned long sigset_t;

struct sigaction {
    void (*sa_handler)(int);
    sigset_t sa_mask;
    int sa_flags;
    void (*sa_restorer)(void);
};

#define SIG_DFL ((void (*)(int))0)
#define SIG_IGN ((void (*)(int))1)
#define SIG_ERR ((void (*)(int))-1)

#ifdef __cplusplus
}
#endif

#endif
