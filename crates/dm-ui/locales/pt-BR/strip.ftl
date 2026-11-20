# Mensagens de strip. Os ids começam com "strip-".
# A barra é a fileira de botões no alto da tela. As palavras precisam ser curtas: quando não
# cabem, os botões mostram só ícones e as palavras viram dica ao passar o mouse.

strip-play = Tocar
# Tocar no modo música: toca a música inteira em vez de um padrão.
strip-play-song = Tocar música
strip-stop = Parar
# Dica no botão Tocar quando a bateria vai para uma saída MIDI em vez dos alto-falantes deste
# aparelho. $device é o nome da saída, como o sistema informa.
strip-midi-tip = A bateria toca em { $device }.
# Dica no botão Tocar enquanto a entrada MIDI está ligada e conectada: tocar um padrão grava as
# batidas dela. $device é o nome da entrada, como o sistema informa.
strip-record-tip = Tocar grava o que vem de { $device }.
# O botão Entrada MIDI, mostrado enquanto a entrada MIDI está conectada. Ligado, tocar um padrão
# grava o que se toca na entrada. $device é o nome da entrada, como o sistema informa.
strip-midi-in = Entrada MIDI
strip-midi-in-off-tip = Gravar de { $device } enquanto um padrão toca.
strip-midi-in-on-tip = Gravando de { $device } enquanto um padrão toca. Toque para parar.
strip-patterns = Padrões
strip-tempo-tip = Andamento e swing
strip-undo = Desfazer
strip-redo = Refazer
strip-menu = Menu

# O botão de compasso, quando a grade mostra um compasso do padrão.
strip-bar = Compasso { $first } de { $total }
# O botão de compasso, quando a grade mostra vários compassos lado a lado.
strip-bars = Compassos { $first }-{ $last } de { $total }
# As setas dos dois lados do botão de compasso: mostram os passos antes ou depois.
strip-back = Voltar
strip-forward = Avançar
# As mesmas setas no primeiro ou no último compasso, onde dão a volta até a outra ponta.
strip-to-end = Ir para o fim
strip-to-start = Voltar ao início

# O botão de intensidade dos passos. A palavra diz por quais intensidades um toque num passo
# passa: só liga e desliga; normal, suave e desligado; ou normal, forte, suave e desligado.
strip-levels-on-off = Simples
strip-levels-soft = Suave
strip-levels-loud = Forte
# A dica do botão de intensidade, uma para cada escolha.
strip-levels-tip-on-off = { strip-levels-on-off }: um toque liga ou desliga o passo. Toque aqui para ter passos suaves também.
strip-levels-tip-soft = { strip-levels-soft }: um toque vai de ligado a suave e a desligado. Toque aqui para ter passos fortes também.
strip-levels-tip-loud = { strip-levels-loud }: um toque vai de ligado a forte, suave e desligado. Toque aqui para só ligar e desligar.
