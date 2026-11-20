# Mensagens de projects. Os ids começam com "projects-".
# Dizem o que salvar, abrir e exportar fizeram. Aparecem no menu ☰ ou na janela Exportar.
# $name é o nome de um projeto como a pessoa o digitou; $path é o caminho completo de um
# arquivo; $reason é o motivo de uma falha, já em palavras.

# O nome que um projeto sem nome recebe, para um arquivo salvo ou uma exportação, e a dica na
# caixa de nome de Salvar como.
projects-default-name = Minha batida

projects-new = Projeto novo iniciado.
projects-saved = "{ $name }" salvo.
projects-not-saved = Não foi salvo: { $reason }
projects-opened = "{ $name }" aberto.
projects-open-failed = "{ $name }" não abriu: { $reason }
projects-deleted = "{ $name }" excluído.
projects-delete-failed = "{ $name }" não foi excluído: { $reason }
projects-opened-file = Arquivo aberto: { $path }.
projects-open-file-failed = { $path } não abriu: { $reason }
projects-saved-file = Salvo em { $path }.
# $name é o nome do estilo, como Rock, que fica como está. "Tocar" é o botão Tocar da barra.
projects-preset-started = Batida { $name } carregada. Toque em Tocar para ouvir.

## Exportações. $place é um caminho completo ou o que projects-saved-where dá.

projects-midi-saved = Arquivo MIDI salvo em { $place }.
projects-midi-failed = O arquivo MIDI não foi salvo: { $reason }
projects-wav-saved = Gravação salva em { $place }.
projects-wav-failed = A gravação não foi salva: { $reason }
projects-style-saved = Estilo para teclado salvo em { $place }.
# $reason é uma mensagem keyboard-problem ou o motivo do sistema para o arquivo não ser gravado.
projects-style-failed = O estilo para teclado não foi salvo: { $reason }
# O nome de um arquivo exportado, depois onde o celular guarda as exportações, como "o app
# Arquivos, em No Meu iPhone, …". Entre parênteses porque o lugar começa com artigo.
projects-saved-where = { $file } ({ $place })
# Motivos de uma gravação não ter sido salva.
projects-export-cancelled = você a interrompeu.
projects-export-nothing = não há nada para tocar.
projects-export-stopped = a gravação parou de repente.

## Os tipos de arquivo que as janelas de abrir e salvar do computador listam.

projects-file-kind-project = Projeto do TickTapDrum
projects-file-kind-midi = Arquivo MIDI
projects-file-kind-wav = Gravação WAV
projects-file-kind-style = Estilo para teclado

# Um estilo enviado ao teclado por MIDI. $name é o nome do arquivo no teclado.
projects-style-sent = Salvo no teclado como { $name }. Escolha em Style, User.
projects-style-send-no-answer = O teclado não respondeu. O envio funciona com teclados Yamaha PSR-SX e Genos.
projects-style-send-not-connected = O estilo não foi enviado: o teclado não está mais conectado.
projects-style-send-no-input = O estilo não foi enviado: o aplicativo não ouve as respostas do teclado nesta saída MIDI.
projects-style-send-refused = O teclado recusou o estilo. Tente de novo, ou salve como arquivo e copie com um pen drive.
# $reason diz o que está errado nas partes escolhidas.
projects-style-send-failed = O estilo não foi enviado: { $reason }
