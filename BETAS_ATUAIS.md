# Betas atuais — Veiga's S3CC Splitter v2.0.0

**Convenção oficial a partir de 22 de outubro de 2026:** o número da versão do aplicativo é **v2.0.0 em ambas as betas**. As diferenças de funcionalidades permanecem identificadas apenas por **Beta 1** e **Beta 2**. A Beta 2 será integrada à Beta 1 antes da versão final 2.0.0; a integração **ainda não foi realizada**.

| Versão exibida na janela | Branch de trabalho oficial | Origem |
| --- | --- | --- |
| `Veiga's S3CC Splitter v2.0.0 | Beta 1` | `beta/v2.0.0-beta1` | Antigas Beta 1 e Beta 2 unificadas; Separador e Mesclador |
| `Veiga's S3CC Splitter v2.0.0 | Beta 2` | `beta/v2.0.0-beta2` | Antiga Beta 3 (código anteriormente chamado v2.1.0/Beta Final 2.1), incluindo poses/CLIP/PoseList |

## Como trabalhar daqui em diante

- Use **somente** `beta/v2.0.0-beta1` e `beta/v2.0.0-beta2` para desenvolvimento, testes e builds novos.
- Não use as antigas `beta-final/v2.0.0`, `beta-final/v2.1.0`, `build/v2.0.0`, `build/v2.1.0` e `build/v2.2.0` como ponto de partida para desenvolvimento novo. Essas referências foram preservadas para consulta e recuperação do histórico.
- Ambas as branches mantêm **v2.0.0** nos metadados do aplicativo (npm, Tauri e Cargo). Diferencie builds sempre pelo rótulo **Beta 1/Beta 2**; não distribua os binários sem esse identificador, pois os dois têm o mesmo número de versão e podem compartilhar identificador de instalação.
- O seletor nativo de arquivos exibe **`*.package`**; aceita arquivos `.package` e `.dbc`, como antes, para não perder compatibilidade.
- Use o `CHANGELOG.md` da própria branch para recursos, mudanças e créditos. **GreatDaySimmer sugeriu o suporte a poses**, incluído na Beta 2.
- O relato de GreatDaySimmer sobre lag em packages criados pelo Mesclador continua **em investigação**, não é uma correção concluída.
- Nunca confunda o nome **Beta 2** com uma versão final **v2.1.0**: o plano atual é lançar uma única versão estável **v2.0.0**, após integrar e validar as funcionalidades.

## Documentos legados

O antigo `BETA_FINAL.md` e as branches com `beta-final` representam a **nomenclatura histórica** anterior a esta decisão. Este documento é a referência atual.
