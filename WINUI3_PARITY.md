# Padrão WinUI 3 — Manager, Packer e Splitter

**Aplicação:** as branches de desenvolvimento/beta do projeto usam a mesma camada visual `src/winui3-strict.css`, carregada **por último** em `index.html`.

## Fonte oficial

Microsoft — [Geometry in Windows 11](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/geometry), [NavigationView](https://learn.microsoft.com/en-us/windows/apps/design/controls/navigationview).

A documentação WinUI 3 utiliza:
- **4 px:** `ControlCornerRadius` — botões, inputs, seletores, listas e superfícies internas, progress bars e scrollbar;
- **8 px:** `OverlayCornerRadius` — diálogos, flyouts e menus sobrepostos, e contêineres de nível superior;
- **0 px:** faces encaixadas — sidebar ligada ao painel de conteúdo e cabeçalhos ligados à lista.

## Componentes cobertos pela camada de paridade

- Sidebar e painel de conteúdo com bordas conectadas e um canto superior de 8 px.
- Lista principal, linhas, preview, cards, drag and drop, painel de resultados e seus raios internos.
- Buttons, SearchBox, TextBox, ComboBox, dropdown de idiomas, menu de temas, seletores, campos e estados de foco.
- ContentDialog/modal, janelas de confirmação, flyout/popover, menus e estados dos controles existentes.
- Progresso e scrollbars Fluent: largura estável do canal, polegar compacto e mudança visual em hover/pressed sem alterar a largura da área rolável.
- CSS com tokens, contraste forçado e redução de movimento. As cores permanecem vinculadas aos **temas de cada programa**.

## Validação

- Teste de contrato: `node --test src/winui3-strict.test.mjs` a partir da pasta do frontend.
- CI: `.github/workflows/winui3-strict-ui.yml` executa o mesmo teste independentemente dos scripts históricos da branch.
- Auditoria estática: confirmar que `winui3-strict.css` é o **último** stylesheet em `index.html`; conferir 4/8/0 px e preservação dos IDs/comandos do programa.
- **Ainda exigido para aprovação visual integral:** testar janela normal/maximizada, alta DPI, oito ou mais temas quando existentes, idiomas EN/PT/ES, teclado, acessibilidade, centenas de itens com scroll, estados de erro/aviso/confirmação, e responsividade em pelo menos 700/920/1280 px.
- A camada CSS **não substitui** a implementação por controles nativos WinUI 3. Estes aplicativos continuam usando suas tecnologias web/Tauri; efeitos do sistema operacional como Mica real e comportamento nativo exato requerem integração ou migração específica para Windows App SDK.
- Conclusão de um teste estático não equivale a aprovação de todas as telas. Qualquer desvio visual ou de interação precisa de correção e teste antes de declarar fidelidade completa.

## Política para próximas betas

Manter `src/winui3-strict.css` e seu teste alinhados entre as versões. Não restabelecer raios históricos (10/11/12/16/18/20/24 px) em superfícies que correspondem aos controles padronizados. Não uniformizar artificialmente indicadores circulares, avatares ou outros símbolos cuja geometria é intencionalmente distinta.

Esta padronização é **visual**: não alterar a lógica de análise, categorização, pack/unpack, Splitter, Merger, classificação, exportação, drag-and-drop ou caminhos de origem dos arquivos.
