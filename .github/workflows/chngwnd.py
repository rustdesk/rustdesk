# flutter_window.cc / RustdeskMultiWindow 
substitutions2 = {
    "RustdeskMultiWindow": "IpmrmtMltWndw",
}

# Nome do arquivo
file_name = r".\flutter\windows\flutter\ephemeral\.plugin_symlinks\desktop_multi_window\windows\flutter_window.cc"

# Inicializa o contador de substituições
num_replacements = 0

# Abre o arquivo para leitura e le os dados
with open(file_name, 'r') as file:
    data = file.read()

# Realiza as substituições e conta quantas foram feitas
for old_string, new_string in substitutions2.items():
    count = data.count(old_string)
    if count > 0:
        print(f" '{old_string}' --> '{new_string}'... ({count})")
        data = data.replace(old_string, new_string)
        num_replacements += count

# Escreve os dados de volta no arquivo
with open(file_name, 'w') as file:
    file.write(data)

# Gera um log na tela com o número total de substituições
print(f"\n===> Total flutter_window.cc {num_replacements} replaced.")
