# Mensagens de errors. Os ids começam com "errors-".

## Arquivos que o sistema não conseguiu ler ou gravar. $detail é a descrição do próprio sistema.

errors-read-file = não foi possível ler o arquivo: { $detail }
errors-write-file = não foi possível gravar o arquivo: { $detail }
errors-delete-file = não foi possível apagar o arquivo: { $detail }
errors-create-folder = não foi possível criar a pasta: { $detail }

## Arquivos de projeto que não abriram.

errors-not-project = Este arquivo não é um projeto do TickTapDrum. Não foi possível lê-lo ({ $detail }).
errors-newer-format = Este arquivo foi feito por uma versão mais nova do TickTapDrum (formato de arquivo { $format }). Atualize o app para abri-lo.
errors-damaged = Este arquivo está danificado ou incompleto: { $detail }.
errors-not-an-object = Este arquivo está danificado ou incompleto: ele não contém um projeto.
errors-format-missing = Este arquivo está danificado ou incompleto: falta o número do formato do arquivo.
errors-format-zero = Este arquivo está danificado ou incompleto: o número do formato do arquivo é 0.
errors-format-not-whole = Este arquivo está danificado ou incompleto: o número do formato do arquivo não é um número inteiro.
errors-no-rows = Este arquivo está danificado ou incompleto: o projeto não tem linhas de bateria.
errors-too-many-rows = Este arquivo está danificado ou incompleto: o projeto tem { $count } linhas de bateria, e o máximo permitido é { $max }.
errors-no-patterns = Este arquivo está danificado ou incompleto: o projeto não tem padrões.
errors-too-many-patterns = Este arquivo está danificado ou incompleto: o projeto tem { $count } padrões, e o máximo permitido é { $max }.
errors-song-too-long = Este arquivo está danificado ou incompleto: a música tem { $count } entradas, e o máximo permitido é { $max }.
errors-duplicate-row-id = Este arquivo está danificado ou incompleto: duas linhas de bateria têm o id { $id }.
errors-bad-pattern-name = Este arquivo está danificado ou incompleto: "{ $name }" não é um nome de padrão, que deve ser uma letra de A a Z.
errors-duplicate-pattern = Este arquivo está danificado ou incompleto: há dois padrões chamados { $name }.
errors-song-pattern-missing = Este arquivo está danificado ou incompleto: a música toca o padrão "{ $name }", que não está no arquivo.
