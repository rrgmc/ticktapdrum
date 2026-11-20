# Mensagens de menu. Os ids começam com "menu-".
# O menu abre sob o botão ☰. Os itens têm cerca de 240 pontos de largura.

## Botões da barra que o menu oferece quando não cabem na barra.

menu-undo = Desfazer
menu-redo = Refazer
menu-tempo-tip = Andamento e swing

## O estado no alto do menu.

# No lugar do nome do projeto, antes de ele ser salvo com um nome.
menu-no-name = Ainda sem nome
menu-unsaved = Alterações ainda não salvas. Mesmo assim, ficam neste aparelho.

## Os itens do menu. Cada um é também o título da janela que abre.

menu-save = Salvar
menu-save-as = Salvar como
menu-open = Abrir
# Começa um projeto novo, vazio.
menu-new = Novo
# Abre uma lista de batidas prontas em estilos conhecidos (rock, funk etc.).
menu-presets = Estilos prontos
# Uma tablatura de bateria é a bateria
# escrita como texto, uma linha por peça, como "SD |----o-------o---|".
menu-paste-tab = Colar uma tablatura de bateria
menu-sounds = Sons
menu-song = Música
menu-export = Exportar
# Trava a tela na horizontal.
menu-landscape = Manter na horizontal
menu-key-names = Notas do teclado
menu-key-names-tip = Com a entrada MIDI ligada, mostra ao lado de cada peça a nota do teclado que a toca.
menu-bar-line = Mostrar compassos abaixo dos botões
menu-language = Idioma
# A primeira escolha da janela Idioma: seguir o idioma do sistema. $language é o nome desse
# idioma nele mesmo, como "English".
menu-language-system = Sistema ({ $language })
# Envia a bateria para outro aparelho ou programa por MIDI. $device é o nome da saída escolhida,
# como o sistema informa e encurtado para caber, ou menu-midi-output-none. O nome pode ser
# masculino ou feminino, então as frases evitam adjetivos que concordam com ele.
menu-midi-output-choice = Saída MIDI: { $device }
# O título da janela que escolhe a saída MIDI.
menu-midi-output = Saída MIDI
# A primeira escolha da janela Saída MIDI: nenhuma saída, então a bateria toca neste aparelho.
menu-midi-output-none = Nenhuma
menu-midi-output-about = A bateria toca na saída escolhida em vez de neste aparelho.
# Uma saída escolhida que não está presente agora. $device é o nome dela, como o sistema informou.
menu-midi-output-not-connected = { $device } (sem conexão)
# Abaixo da lista, quando a saída escolhida aparece na lista mas não está conectada (ainda).
menu-midi-output-waiting = { $device } ainda está sem conexão.
# O controle de volume abaixo da lista, mostrado enquanto há uma saída escolhida.
menu-midi-volume = Volume
menu-midi-volume-about = A saída escolhida toca todas as peças neste volume. O volume de cada peça continua valendo.
# Mensagens de uma linha no estado do menu. $device é o nome da saída.
menu-midi-sending = Enviando para { $device }.
menu-midi-gone = { $device } não está mais disponível. Tocando neste aparelho de novo.
menu-midi-in-use = { $device } está em uso por outro programa.
# Mensagem de uma linha quando a entrada MIDI some durante a gravação. $device é o nome dela.
menu-midi-input-gone = { $device } não está mais disponível. A gravação parou.
# Aviso quando o teclado troca o kit de bateria com a entrada MIDI ligada. $kit é o nome do kit
# como o teclado mostra, como ElectroKit. O botão grooves-use-kit pode vir em seguida.
menu-midi-kit = Kit do teclado: { $kit }
# O mesmo, para um teclado que dá ao kit um número, mas nenhum nome. O botão grooves-use-kit vem
# em seguida.
menu-midi-kit-other = O teclado trocou o kit de bateria.
# De onde vêm as batidas tocadas num aparelho MIDI. $device é o nome da entrada escolhida, como o
# sistema informa e encurtado para caber, ou menu-midi-input-same.
menu-midi-input-choice = Entrada MIDI: { $device }
# O título da janela que escolhe a entrada MIDI.
menu-midi-input = Entrada MIDI
# A primeira escolha da janela Entrada MIDI: a entrada do mesmo aparelho da saída MIDI.
menu-midi-input-same = Igual à saída
menu-midi-input-about = De onde vêm as batidas que você toca. Igual à saída serve para um teclado; escolha outra para pads de bateria.
# Uma entrada escolhida que não está presente agora. $device é o nome dela, como o sistema informou.
menu-midi-input-not-connected = { $device } (sem conexão)
# O título da janela que ajusta o andamento e o swing.
menu-tempo = Andamento

## Perguntas feitas antes de perder algo não salvo.

menu-new-question = Começar uma batida nova? Esta batida tem alterações não salvas, e elas se perdem.
menu-start-new = Novo
# Responde "não" a uma pergunta que faria perder as alterações desta batida.
menu-keep-this = Cancelar
menu-open-file-question = Abrir um arquivo? As alterações não salvas desta batida se perdem.
menu-choose-file = Escolher arquivo
menu-open-question = Abrir "{ $name }"? As alterações não salvas desta batida se perdem.
# $name é o nome de um projeto salvo, como a pessoa o digitou.
menu-delete-question = Excluir "{ $name }" de vez?
menu-delete = Excluir
# Responde "não" a "Excluir de vez?".
menu-keep = Cancelar
menu-cancel = Cancelar

## A janela Abrir.

# Só no computador: abre um arquivo de projeto de qualquer pasta.
menu-open-file = Abrir arquivo
# Só no computador: salva o projeto como arquivo em qualquer pasta.
menu-save-file = Salvar em arquivo
menu-nothing-saved = Nada salvo ainda.

## A janela Salvar como.

# O rótulo acima da caixa onde se digita o nome do projeto.
menu-name = Nome
# $name é o nome digitado, que um projeto salvo já tem.
menu-replaces = Isto substitui o "{ $name }" já salvo.

## A janela Estilos prontos. Os nomes dos estilos, como Rock, ficam como estão.

menu-presets-intro = Uma batida pronta para tocar e mudar. Ela substitui a batida da tela.
menu-preset-question = Carregar { $name }? As alterações não salvas desta batida se perdem.
menu-start = Carregar
menu-preset-rock = Uma batida reta e firme, com o bumbo no 1 e no 3, a caixa no 2 e no 4 e uma virada a cada quatro compassos.
menu-preset-pop = Uma batida viva e regular, com caixa e palmas juntas no 2 e no 4 sobre um chimbal movimentado.
menu-preset-hip-hop = Um groove boom bap descontraído, com bumbo pesado, caixa seca e um pouco de swing.
menu-preset-funk = Um groove cheio, com bumbo sincopado, chimbal acentuado e notas fantasma suaves na caixa.
menu-preset-bossa-nova = Um ritmo brasileiro suave, com bumbo balançado e clave leve na caixa ao longo de dois compassos.
menu-preset-house = Uma batida dançante, com bumbo em todo tempo, palmas no 2 e no 4 e chimbal aberto entre eles.
menu-preset-reggae = Um one drop tranquilo, com bumbo e caixa juntos no tempo 3 e chimbal em shuffle.

## A janela Exportar.

menu-recording = Gravando a música num arquivo WAV
# Interrompe a gravação em andamento.
menu-stop = Parar
menu-midi = Arquivo MIDI
menu-midi-about = As notas, para outros programas de música.
menu-wav = Gravação WAV
menu-wav-about = O som, para tocar em qualquer player.
# Aparece enquanto os sons da bateria ainda estão baixando, com o botão WAV desativado.
menu-wav-about-no-bank = O som, para tocar em qualquer player. Precisa dos sons da bateria, que ainda não foram carregados.
# $place é onde um celular põe os arquivos exportados, como "o app Arquivos, em No Meu iPhone, …".
menu-export-place = Local das exportações: { $place }.
# Um título pequeno sobre os dois botões de estilo para teclado. Um teclado arranjador toca um
# "estilo" de acompanhamento que segue os acordes de quem toca.
menu-for-keyboard = Para um teclado
# Um botão. Sempre "Estilo para teclado", nunca só "Estilo": o app já chama suas batidas prontas
# de estilos. Também é o título da etapa que escolhe as seções.
menu-keyboard-style = Estilo para teclado
# A linha sob Estilo para teclado, para teclados Yamaha. "User" é o nome do botão do teclado,
# como vem impresso nele. Só estas linhas citam marca; os botões nunca.
menu-keyboard-style-about-yamaha = Um estilo para teclado arranjador Yamaha, como o PSR-SX720. Copie para um pen drive e escolha em User.
# Um botão que envia o estilo direto ao teclado. Também é o título da etapa que escolhe as seções.
menu-keyboard-send = Enviar ao teclado
menu-keyboard-send-about = Põe o estilo direto nos estilos User do teclado, pela saída MIDI.
# Sob Enviar ao teclado enquanto ele está desativado porque nenhuma saída MIDI está conectada.
# "Ajustes" e "Saída MIDI" são o grupo e o item do menu.
menu-keyboard-send-about-no-output = Escolha antes o teclado em Ajustes, Saída MIDI.

## O grupo Ajustes no fim do menu.

menu-settings = Ajustes
menu-import = Importar
# Como cada linha aparece na grade: pelo nome, por um desenho da peça, ou pelos dois.
menu-rows-names = Instrumentos: nomes
menu-rows-pictures-and-names = Instrumentos: desenhos e nomes
menu-rows-pictures = Instrumentos: desenhos

## A janela Colar uma tablatura de bateria. O título dela é menu-paste-tab.

menu-paste-tab-hint = Cole alguns compassos de uma tablatura de bateria, com o nome da peça no começo de cada linha.
# Adiciona a tablatura colada ao projeto como um padrão novo.
menu-paste-tab-add = Adicionar como padrão
menu-paste-tab-skip-odd-bars = Deixar de fora os compassos que não cabem
menu-paste-tab-sources = Onde encontrar tablaturas de bateria
menu-paste-tab-source-ultimate-guitar = Ultimate Guitar: tablaturas de bateria
menu-paste-tab-source-tabcrawler = Tabcrawler

# Mostrado com uma barra de progresso enquanto um estilo vai para o teclado por MIDI.
menu-keyboard-sending = Enviando o estilo ao teclado
