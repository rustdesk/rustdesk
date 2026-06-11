import os
import shutil

def remove_line_block(filepath, start_phrase, lines_to_remove_after_start):
    """
    Removes a starting line and a fixed number of lines immediately following it.

    :param filepath: The path to the file to modify.
    :param start_phrase: The unique string to identify the first line of the block.
    :param lines_to_remove_after_start: The number of lines to remove after the starting line.
    """
    
    # 1. Configuration for the removal logic
    # The starting line is: const KEY: &str = "5Qbwsde3unUcJBtrx9ZkvUmwFNoExHzpryHuPUdqlWM=";
    # The block contains this line plus 8 following lines, so we want to skip 9 lines in total.
    total_lines_to_skip = 1 + lines_to_remove_after_start # 1 (start line) + 8 (following lines) = 9
    
    lines_to_keep = []
    skip_count = 0

    # 2. Read and filter the file content
    try:
        with open(filepath, 'r') as file:
            for line in file:
                
                # If we are currently in the process of skipping lines, decrement the counter and continue
                if skip_count > 0:
                    skip_count -= 1
                    continue
                
                # Check if the line matches the start phrase (we use .strip() to ignore indentation/whitespace)
                if line.strip().startswith(start_phrase.strip()):
                    # Start skipping the block (including the current line)
                    skip_count = total_lines_to_skip - 1 
                    # Note: We subtract 1 because the 'continue' will handle the first line removal immediately
                    continue 

                # If we are not skipping, keep the line, but change custom.txt to custom_.txt
                line = line.replace("custom.txt", "custom_.txt")
                lines_to_keep.append(line)
                
    except FileNotFoundError:
        print(f"Error: File not found at {filepath}")
        return

    # 3. Write the remaining lines back to the file
    try:
        with open(filepath, 'w') as file:
            file.writelines(lines_to_keep)
            
        print(f"Success! Removed the 9-line block starting with '{start_phrase.strip()}' from {filepath}.")
        
    except IOError as e:
        print(f"An error occurred while writing to the file: {e}")

def main():
    file_path = 'src/common.rs' 
    start_phrase = 'const KEY: &str = "5Qbwsde3unUcJBtrx9ZkvUmwFNoExHzpryHuPUdqlWM=";'
    lines_to_remove_after_start = 8 
    remove_line_block(file_path, start_phrase, lines_to_remove_after_start)

if __name__ == "__main__":
    main()