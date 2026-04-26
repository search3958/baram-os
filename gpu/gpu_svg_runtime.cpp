#include <__config>
#include <cstddef>
#include <cstdlib>
#include <new>

void* operator new(std::size_t size)
{
    if(size == 0)
        size = 1;
    void* ptr = std::malloc(size);
    if(ptr == nullptr)
        std::abort();
    return ptr;
}

void* operator new[](std::size_t size)
{
    return ::operator new(size);
}

void* operator new(std::size_t size, std::align_val_t)
{
    return ::operator new(size);
}

void* operator new[](std::size_t size, std::align_val_t)
{
    return ::operator new(size);
}

void* operator new(std::size_t size, const std::nothrow_t&) noexcept
{
    if(size == 0)
        size = 1;
    return std::malloc(size);
}

void* operator new[](std::size_t size, const std::nothrow_t&) noexcept
{
    return ::operator new(size, std::nothrow);
}

void operator delete(void* ptr) noexcept
{
    std::free(ptr);
}

void operator delete[](void* ptr) noexcept
{
    std::free(ptr);
}

void operator delete(void* ptr, std::size_t) noexcept
{
    std::free(ptr);
}

void operator delete[](void* ptr, std::size_t) noexcept
{
    std::free(ptr);
}

void operator delete(void* ptr, std::align_val_t) noexcept
{
    std::free(ptr);
}

void operator delete[](void* ptr, std::align_val_t) noexcept
{
    std::free(ptr);
}

void operator delete(void* ptr, std::size_t, std::align_val_t) noexcept
{
    std::free(ptr);
}

void operator delete[](void* ptr, std::size_t, std::align_val_t) noexcept
{
    std::free(ptr);
}

void operator delete(void* ptr, const std::nothrow_t&) noexcept
{
    std::free(ptr);
}

void operator delete[](void* ptr, const std::nothrow_t&) noexcept
{
    std::free(ptr);
}

_LIBCPP_BEGIN_NAMESPACE_STD

[[noreturn]] void __libcpp_verbose_abort(const char*, ...) noexcept
{
    std::abort();
    __builtin_unreachable();
}

_LIBCPP_END_NAMESPACE_STD

extern "C" void __cxa_pure_virtual(void)
{
    std::abort();
}
