#include <__config>
#include <__memory/shared_count.h>
#include <string>
#include <typeinfo>

_LIBCPP_BEGIN_NAMESPACE_STD

__shared_count::~__shared_count() = default;
__shared_weak_count::~__shared_weak_count() = default;

void __shared_weak_count::__release_weak() noexcept
{
    if(__libcpp_atomic_refcount_decrement(__shared_weak_owners_) == -1)
        __on_zero_shared_weak();
}

__shared_weak_count* __shared_weak_count::lock() noexcept
{
    if(__shared_owners_ == -1)
        return nullptr;
    __add_shared();
    return this;
}

const void* __shared_weak_count::__get_deleter(const type_info&) const noexcept
{
    return nullptr;
}

_LIBCPP_END_NAMESPACE_STD

template class std::basic_string<char>;
template class std::basic_string<char32_t>;
