# A lista de grooves e a importação de grooves. Os ids começam com "grooves-".
# Um groove é um trecho curto de bateria que se repete, vindo de um arquivo MIDI. Um pacote é
# um zip com muitos desses arquivos, ou um arquivo só.

## Os itens do menu ☰ e a janela da lista de grooves.

grooves-add-from-file = Adicionar grooves de um arquivo
grooves-title = Grooves
grooves-intro = Toque em Tocar para ouvir um groove. Adicionar coloca o groove no projeto como um padrão novo.
grooves-empty = Nenhum groove ainda.
grooves-pack = { $name } ({ $count })
grooves-bars = { $count ->
    [one] 1 compasso
   *[other] { $count } compassos
}
grooves-play = Tocar
grooves-stop = Parar
grooves-add = Adicionar
grooves-add-tip = Adicionar como um padrão novo
grooves-delete = Apagar
grooves-keep = Cancelar
grooves-delete-pack-question = Apagar todos os grooves de { $name }?
grooves-delete-groove-question = Apagar o groove { $name }?
grooves-delete-failed = Não foi apagado: { $reason }
grooves-file-kind = Arquivos MIDI e zips

## A janela Adicionar grooves de um link.

grooves-add-from-link = Adicionar grooves de um link
grooves-link-hint = Cole um link para um arquivo de groove (.mid) ou um pacote (.zip).
grooves-link-add = Adicionar
grooves-link-downloading-kb = Baixando... { $kb } KB
grooves-link-downloading-mb = Baixando... { $mb } MB
grooves-link-not-a-link = Isto não é um link.
grooves-link-failed = O download falhou. Confira o link e a conexão.
grooves-link-status = O servidor respondeu com um erro ({ $code }).
grooves-link-too-large = O arquivo é grande demais.
grooves-link-web-page = Isto é uma página da web, não um arquivo de groove. Copie o link do próprio download, muitas vezes tocando e segurando o botão de download.
grooves-link-share-refused = Só links para arquivos de grooves podem ser compartilhados com o TickTapDrum.

## O que uma importação fez.

grooves-added = { $count ->
    [one] 1 groove adicionado de { $name }.
   *[other] { $count } grooves adicionados de { $name }.
}
grooves-left-out = { $refused ->
    [one] 1 arquivo ficou de fora: { $reasons }.
   *[other] { $refused } arquivos ficaram de fora: { $reasons }.
}
grooves-only-general-midi = Só a versão General MIDI foi usada.
grooves-not-added = { $name } não foi adicionado: { $reason }.
grooves-none-added = Nada foi adicionado de { $name }. { $refused } arquivos ficaram de fora: { $reasons }.
grooves-none-found = Nada foi adicionado de { $name }: não há grooves nele.
grooves-reason-count = { $reason } ({ $count })
grooves-read-failed = Não deu para ler { $name }: { $reason }
grooves-not-saved = Os grooves não foram guardados: { $reason }

## Por que um arquivo não é um groove que o app aceita, em palavras simples.

grooves-refusal-not-midi = não é um arquivo MIDI
grooves-refusal-no-notes = não tem notas
grooves-refusal-several-channels = é uma música inteira, não um groove
grooves-refusal-not-four-four = não está em 4/4
grooves-refusal-tempo-changes = o andamento muda
grooves-refusal-too-long = tem mais de 16 compassos
grooves-refusal-off-grid = tocado livre, fora da grade
grooves-refusal-other-drum-program = feito para outro programa de bateria (tente a versão General MIDI do pacote)

## Por que um arquivo não pôde ser importado.

grooves-error-not-grooves = { $name } não é um arquivo MIDI nem um zip.
grooves-error-too-large = { $name } é grande demais.
grooves-error-broken-zip = { $name } é um zip danificado.
grooves-error-no-midi-files = { $name } não tem arquivos MIDI.

## O que adicionar um groove ao projeto fez.

grooves-added-pattern = { $bars ->
    [one] Padrão { $pattern } criado: { $name }, 1 compasso.
   *[other] Padrão { $pattern } criado: { $name }, { $bars } compassos.
}
grooves-repeats = O arquivo repete { $bars ->
    [one] o mesmo compasso
   *[other] os mesmos { $bars } compassos
} nos seus { $given } compassos, então o padrão fica só com { $bars ->
    [one] ele
   *[other] eles
}.
grooves-bars-left-out = { $count } compassos depois dos 16 primeiros ficaram de fora.
grooves-rows-added = { $count ->
    [one] 1 linha foi adicionada para uma peça nova.
   *[other] { $count } linhas foram adicionadas para peças novas.
}
grooves-drums-left-out = { $count } peças ficaram de fora: o projeto não tem espaço para mais linhas.
grooves-no-room = Não há espaço para outro padrão: um projeto comporta 26.
grooves-add-failed = O groove não foi adicionado.
