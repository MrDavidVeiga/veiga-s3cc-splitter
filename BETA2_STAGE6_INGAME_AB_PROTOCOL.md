# Etapa 6 — Beta 2 / GreatDaySimmer: protocolo A/B no The Sims 3

**Situação:** protocolo pronto; **testes dentro do jogo NÃO executados por este documento**.
**Commit/EXE aprovado na CI:** `1677f38b99d47bab550ae75b798d095b2275426e`.
**GitHub Actions (Windows x64):** https://github.com/MrDavidVeiga/veiga-s3cc-splitter/actions/runs/40555409594
**Suite Rust/JS + comparador:** https://github.com/MrDavidVeiga/veiga-s3cc-splitter/actions/runs/40555409628
**PR experimental:** https://github.com/MrDavidVeiga/veiga-s3cc-splitter/pull/32 (draft; nunca mesclar automaticamente).

## O que está provado e o que falta

A Etapa 5, executada pelo Codex com 19 skins reais, aprovou Merge→Unmerge: 359/359 recursos, 0 TGIs perdidos/extras, hashes decodificados idênticos, 29 TONEs com as mesmas dependências, 282 IMG e 29 ramp PNGs íntegros e arquivos originais preservados. Os 4 NMAP reconstruídos normalizaram compressão, mas mantiveram os bytes decodificados. O merged de saída da Etapa 5 foi reportado com SHA-256 `F207EC41E6A8596E2DE5C7D2D57CDE41FAD9249E1F800B130243F1FB2BAEACDF`.

Os dois testes sintéticos de conflito (recusa de TGI divergente; diagnóstico de IMG e preservação das origens) **passaram no GitHub Actions** do mesmo commit. O bloqueio `aho-corasick v1.1.4` foi local/offline no Codex, não uma falha da CI Windows.

Ainda NÃO temos: resultados dentro do The Sims 3, validação pós-correção de *objetos* em ciclo completo ou demonstração de que o lag observado pela GreatDaySimmer foi corrigido.

## Fontes verificadas no Google Drive

**Coleção SKINS**
- Pasta da testadora: https://drive.google.com/drive/folders/1DiaLI9KfxL7B2zenIcG0XIR550sFlllN
- 19 skins originais: https://drive.google.com/drive/folders/1Q-mwG0kdpgkpn8aprWanFBvogEHh_bZA
- Merge s3pe da mesma coleção: `Non default Skins ORIGINAL Merged with S3pe.package` (463,872,826 bytes), https://drive.google.com/file/d/1IvxAmJm5NDCASBvguD0-wKtJoelrHWFW/view?usp=drivesdk
- Merge criado com a ferramenta anterior: `Non default Skins MERGED with the tool.package` (460,407,376 bytes), https://drive.google.com/file/d/1cIlwJqbjtXSCDl1r-_-pARd-bqVER2Vo/view?usp=drivesdk
- **Candidato a testar:** o NOVO merged gerado na Etapa 5 pelo EXE experimental, e não presumir que o merge antigo prova comportamento da nova versão. Pedir caminho local real ao Codex; confirmar SHA-256 antes de testar.

**Coleção OBJECTS — teste independente da coleção de skins**
- Originais: https://drive.google.com/drive/folders/1ZQImhtPGmvoOHUptaomL97AHFVt3Z3JK
  - `Native_Body_Wash_MariAlison.package` (2,317,554 bytes)
  - `Foot_Callus_Remover_MariAlison.package` (779,479 bytes)
  - `Martassimsbook_4to3_novvvas_LindoSet_Slippers.package` (345,779 bytes)
- Merge anterior: `Merged Objects.package` (3,485,087 bytes), https://drive.google.com/file/d/1qlEiPFalrj8xLsJp3QsRNeMO4LzGVDwq/view?usp=drivesdk
- Unmerge anterior: https://drive.google.com/drive/folders/1vJ3j3Tc9vBmBgJSTCIsJa8I1J_buYPGY

**Atenção:** estes objetos não substituem um teste independente de *duas variantes de quadros* (parede vs chão) se a GreatDaySimmer tiver seus arquivos originais. Não chamar itens visualmente parecidos de duplicatas sem comparar OBJD/catalog TGI.

## Preparação — Codex pode fazer esta parte, sem iniciar o jogo

1. Confirmar no ambiente local o executável experimental correspondente ao commit e aos hashes registrados no relatório da Etapa 5. Nunca usar um binário mais antigo por engano.
2. Solicitar ao operador os caminhos locais reais (links do Google Drive não são caminhos do Windows).
3. Criar uma pasta de trabalho NOVA, fora de `Documents\Electronic Arts\The Sims 3\Mods` e fora dos diretórios dos CCs originais, com subpastas `A_s3pe`, `B_splitter_new`, `C_originals_optional`, `objects_new`, `results`. Não sobrescrever nem apagar pastas existentes.
4. Copiar os .package para suas respectivas pastas. Preparar **uma** variante ativa por cenário:
   - A: merged s3pe das 19 skins;
   - B: NOVO merged produzido na Etapa 5 com as mesmas 19 skins;
   - C (controle opcional): 19 originais não mesclados.
   - Testes de objetos: gerar um NOVO merge experimental dos três objetos originais e então um NOVO unmerge, comparando TGIs, hashes e ICON/THUM com os três originais; usar cópias e arquivos de saída separados. **Não reaproveitar** o antigo `Merged Objects.package` como prova de integridade do novo build.
5. Gerar um inventário CSV de cada cópia (nome, tamanho, SHA-256, caminho local, variante). Verificar que nenhuma cópia contém a mesma coleção duplicada como packages separados e merged.
6. Preparar uma planilha/CSV com as colunas da seção "Registro"; preencher identificadores e hashes antes dos testes, sem inventar resultados de FPS.
7. **Não escrever nada automaticamente em Mods**, não apagar cache nem saves e não iniciar o jogo; a troca de variantes, backup e execução são ações do jogador/testador.

## Teste A/B — conduzido no jogo por David ou GreatDaySimmer

**Antes:** backup separado dos saves, configuração e `Mods`; confirmação de que o jogo não está aberto. Testar em save/lot copiado e de preferência sem salvá-lo; não sobrescrever o save principal. Não editar, apagar ou mover arquivos externos à pasta de testes sem autorização.

Para cada variante:
1. Garantir que **apenas uma versão da coleção** está ativa; nenhum dos outros merges ou originais redundantes pode permanecer em uma segunda pasta de mods. Preservar os mods comuns iguais nas duas execuções.
2. Mesma máquina, versão/patch do jogo, save, lote, hora/situação no save, câmera/rota, configurações gráficas, resolução, expansão e coleção de CCs fora do grupo avaliado.
3. Cache: manter procedimento **idêntico** em todas as rodadas (não limpar somente um cenário). Se optar por limpar, fazer backup e registrar exatamente quais caches foram removidos; não automatizar.
4. Fazer pelo menos **duas execuções por variante**, preferencialmente em ordem A→B→B→A ou B→A→A→B para reduzir efeitos de cache/aquecimento. Registrar reinicialização do jogo em cada execução.
5. Cronometrar: clique para iniciar → menu, carregar save → entrada no lote, carregar CATÁLOGO/CAS, retorno ao jogo; registrar método de medida. Para travamentos/lag, escolher trajeto reproduzível de câmera no mesmo lote, registrar duração, stutter e, se houver medidor, FPS médio/baixo percentil ou frametime (nome da ferramenta usada).
6. Procurar especificamente: 29 skins TONE/IMG em CAS, opções non-default, aparência/textura, nomes e miniaturas; objetos visíveis, nome no Build/Buy, localização em categorias, posicionamento parede/chão, ICON/THUM e textura ao colocar. Capturar screenshots/observações, nunca assumir que toda skin precisa ter um IMG local (dependências externas existem).
7. Em caso de erro de leitura, sumiço de item, lag forte, crash ou erro de thumbnail: registrar variante, save, tempo, arquivo, TGI se possível, print e passos de reprodução. Não corrigir abrindo e salvando o package original.
8. Se possível, repetir sem o merge do Splitter (controle C), mantendo todo o restante igual. Se apenas uma das condições apresentar lag, repetir a condição suspeita antes de atribuir causalidade.

## Registro de resultados

Uma linha por variante e repetição, no arquivo `results/stage6-game-ab.csv` com cabeçalho:

```csv
date,tester,game_patch,save_copy,lot,scenario,trial,merged_sha256,cache_protocol,launch_seconds,load_save_seconds,cas_seconds,catalog_seconds,avg_fps,p1_fps,stutter_count,skins_ok,objects_ok,icons_ok,notes,evidence_path
```

Valores desconhecidos devem ficar em branco, jamais preenchidos com zero falso. Anexar detalhes de cada defeito e os passos para reproduzir. Não publicar screenshots contendo informação pessoal ou CCs de terceiros sem autorização.

## Condições de aprovação

- **Bloco técnico objects:** novo Merge/Unmerge dos 3 objetos sem perda de recurso, TGI alterado, ICON/THUM indevidamente herdado ou entrada OBJD eliminada; testes específicos de variantes de catálogo conforme arquivos disponibilizados.
- **Bloco jogo skins:** mesma coleção e contexto reproduzível; todos os itens verificados presentes/funcionais e sem regressão visual.
- **Bloco jogo desempenho:** sem lag/stutter significativamente pior em B que em A no cenário reproduzível; idealmente reproduzir e comparar o relato original da GreatDaySimmer.
- **Bloco UI/segurança:** nomes, miniaturas, backups e comportamento de erro validados, sem dano a originais ou saves.
- **Bloco release:** nenhum erro crítico aberto e suíte Windows/JS/Rust aprovada no EXATO commit candidato.

**Aprovação da Etapa 5 não aprova a Etapa 6.** PR #32 deve permanecer draft até que os logs de teste humano sejam revisados; NÃO fazer merge nem release automaticamente.
