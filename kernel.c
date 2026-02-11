void kmain(void) {
    const char *str = "Hello, BaramOS!";
    char *vidptr = (char*)0xb8000; // ビデオメモリの開始アドレス
    unsigned int i = 0;
    unsigned int j = 0;

    // 画面をクリア
    while(j < 80 * 25 * 2) {
        vidptr[j] = ' ';
        vidptr[j+1] = 0x07; // 黒背景・白文字
        j = j + 2;
    }

    // 文字列を書き込む
    j = 0;
    while(str[i] != '\0') {
        vidptr[j] = str[i];
        vidptr[j+1] = 0x07;
        i++;
        j = j + 2;
    }
    return;
}