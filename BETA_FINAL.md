# VERSOES BETA FINAIS — ATUAIS

| Versao de trabalho | Identificacao no aplicativo | Branch canonica | Origem |
| --- | --- | --- | --- |
| **Versao Beta Final 2.0** | **Beta 1 — v2.0.0** | `beta-final/v2.0.0` | Uniao da antiga Beta 1 com a antiga Beta 2, usando o Separador e o Mesclador |
| **Versao Beta Final 2.1** | **Beta 2 — v2.1.0** | `beta-final/v2.1.0` | Antiga Beta 3, renumerada sem remover funcionalidades |

## Para trabalhar nas versoes atuais

Use **somente** `beta-final/v2.0.0` para o ramo 2.0 e `beta-final/v2.1.0` para o ramo 2.1. Nao baseie alteracoes novas nas branches historicas `build/v2.0.0`, `build/v2.1.0` ou `build/v2.2.0`, salvo quando fizer auditoria historica ou portar uma correcao com comparacao de codigo.

- Beta 1, Versao Beta Final 2.0: Splitter + Merger; antiga Beta 2 unificada com a Beta 1.
- Beta 2, Versao Beta Final 2.1: recursos da antiga Beta 3, incluindo carregamento otimizado e manutencao de packages mesclados.
- Preservar o historico das antigas branches. Nunca sobrescrever uma branch historica para atualizar a branch canonica.
- Nome 'Beta Final' identifica a linha de desenvolvimento atual. Nao significa compilacao ou testes finais aprovados ate que o CI valide a branch correspondente.

Veja o README de cada branch para os recursos tecnicos e os testes especificos.
