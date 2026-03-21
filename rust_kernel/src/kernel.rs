pub unsafe fn append_uint(mut p: *mut u8, mut v: u32) -> *mut u8 {
    let mut tmp = [0u8; 10];
    let mut n = 0usize;

    if v == 0 {
        *p = b'0';
        return p.add(1);
    }

    while v > 0 && n < tmp.len() {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }

    while n > 0 {
        n -= 1;
        *p = tmp[n];
        p = p.add(1);
    }

    p
}
