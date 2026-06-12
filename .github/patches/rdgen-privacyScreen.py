import os

def convert_png_to_cpp(input_file, output_file, array_name="g_img"):
    if not os.path.exists(input_file):
        print(f"Error: {input_file} not found.")
        return

    with open(input_file, "rb") as f:
        data = f.read()

    with open(output_file, "w") as f:
        f.write('#include "pch.h"\n')
        f.write('#include "./img.h"\n\n')
        f.write(f"const unsigned char {array_name}[] = {{\n")

        for i in range(0, len(data), 20):
            chunk = data[i : i + 20]
            hex_chunk = [f"0x{b:02x}" for b in chunk]
            
            line = ", ".join(hex_chunk)
            
            if i + 20 < len(data):
                f.write(f"{line},\n")
            else:
                f.write(f"{line}\n")

        f.write("};\n\n")
        f.write(f"const long long {array_name}Len = sizeof({array_name});\n")

    #print(f"Successfully converted {input_file} to {output_file}")

convert_png_to_cpp("privacy.png", "img.cpp")