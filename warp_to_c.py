import os

def convert_warp_to_c(warp_file, c_file):
    with open(warp_file, 'r', encoding='utf-8') as f:
        content = f.read()

    # Escape backslashes and quotes for C string
    escaped_content = content.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n"\n"')

    c_code = f'#include "ui_data.h"\n\nconst char *const warp_ui_code =\n"{escaped_content}";\n'

    with open(c_file, 'w', encoding='utf-8') as f:
        f.write(c_code)

if __name__ == "__main__":
    warp_path = "warp_ui/main.warp"
    c_out_path = "warp_ui/ui_data.c"
    
    if os.path.exists(warp_path):
        convert_warp_to_c(warp_path, c_out_path)
        print(f"Converted {warp_path} to {c_out_path}")
    else:
        print(f"Error: {warp_path} not found")
