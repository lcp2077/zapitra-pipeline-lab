# ZAPITRA

**Zero-copy API Pipeline Integrity, Trace & Resilience Analyzer**

ZAPITRA é um modelo de referência independente, escrito em Rust, com explorador limitado de espaços de estados para a API `Pipeline` proposta pelo BLAKE3-Servil. O foco é ownership, ciclo de vida de mensagens e salas, eventos de conclusão, cancelamento e classificação de esperas que dependem de workers ou de ações do chamador.

> **Estado: ferramenta de revisão de design.** O documento upstream de 10 de novembro de 2026 descreve um design, não um runtime implementado. O esqueleto contém corpos `todo!()`. Portanto, ZAPITRA não executa o hashing BLAKE3 por meio de `Pipeline`, não mede zero-copy ou zero-allocation, não faz benchmark, não prova segurança de memória e não testa `io_uring`.

## Objetivo

O design atual dá acesso exclusivo a um único driver mutável do `Pipeline`. `Message`, `Room` e `Event` são valores. Os dados podem entrar por salas confirmadas com `commit`, buffers cuja propriedade é transferida, fatias emprestadas protegidas por `Lent` ou lotes. Os registros de mensagem, eventos concluídos e buffers de sala têm capacidades limitadas.

ZAPITRA fornece um modelo executável externo para discutir esses contratos. Ele não duplica um runtime que ainda não existe.

## O que verifica

- Capacidades separadas para registros de mensagens e eventos concluídos.
- Aquisição de sala antes de conhecer a mensagem de destino.
- Associação da sala somente no `commit`.
- Exclusividade dos caminhos `rooms`, `owned`, `lent` ou `batch`.
- Tags repetidas com IDs internos de mensagem distintos.
- Entrega de eventos na ordem de conclusão.
- Observação de `Message` descartada na próxima chamada ao Pipeline.
- Devolução de sala não confirmada ao pool.
- `Lent` limitado ao tempo de trabalho: descarte antecipado cancela trabalho não iniciado; após todos os leitores terminarem e o evento terminal ser materializado, a guarda pode ser liberada mesmo que `next_event()` ainda não o tenha entregue.
- Devolução conjunta de `Batch<'static>` e do armazenamento de digests.
- Classificação das esperas de mensagem, sala e `ready()`.
- Busca de alcançabilidade limitada apenas às ações dos workers.
- Verificação de origem dos tokens nas operações do modelo.
- Invariantes conferidas após cada transição explorada.

O explorador usa busca em largura (BFS). O relatório registra estados, transições, cada verificação de invariantes bem-sucedida em estados e transições, classes de espera e caminhos testemunha encontrados dentro dos limites configurados.
O código Rust inclui 28 testes unitários para o modelo e o explorador.

## Referências upstream

- Design: <https://github.com/servil-software-factory/BLAKE3/blob/design/one-stream-api/docs/pipeline-api.md>
- Esqueleto de compilação: <https://github.com/servil-software-factory/BLAKE3/tree/design/one-stream-api/docs/experiments/pipeline-api-skeleton>
- Discussão: <https://github.com/servil-software-factory/BLAKE3/issues/356>
- Planejamento da implementação: <https://github.com/servil-software-factory/BLAKE3/issues/357>

## Compilar e executar

Requisitos: Rust stable 1.85 ou superior, Cargo, rustfmt, Clippy, Git, Bash, Python 3, `zip` e `sha256sum`. O projeto não possui dependências Cargo de terceiros. Os scripts utilizam ferramentas já instaladas no Linux e não instalam pacotes nem alteram configurações de rede.

```bash
chmod +x scripts/*.sh
./scripts/build.sh
./scripts/verify.sh
```

O executável será `target/release/zapitra`.

```bash
./target/release/zapitra doctor
./target/release/zapitra selfcheck --report reports/selfcheck.json
./target/release/zapitra explore --depth 9 --states 20000 --report reports/explore.json
./scripts/check-upstream-skeleton.sh
```

Use `zapitra help` para consultar as opções. Os limites padrão do modelo são dois registros de mensagem, duas salas, profundidade oito e 12.000 estados. Aumente os limites com cuidado em máquinas com pouca memória.

## Como interpretar os resultados

`PASS_BOUNDED_MODEL_ORACLES` significa que o espaço finito selecionado foi percorrido sem divergência de invariantes ou dos oráculos de espera. `PASS_BOUNDED_MODEL_ORACLES_WITH_LIMIT` significa que o limite de profundidade ou de estados interrompeu a exploração. Um resultado limitado vale somente para o modelo e os limites registrados.

O script upstream clona a branch atual de design, registra o SHA completo e executa os testes de documentação do esqueleto. Esses testes verificam exemplos aceitos e rejeitados pelo compilador. Os corpos `todo!()` significam que eles não exercitam hashing em runtime.

## Hipóteses explícitas e dúvidas para os mantenedores

1. **Pool de eventos cheio:** o design informa que existe um pool limitado de eventos concluídos, mas não especifica onde fica o resultado enquanto esse pool está cheio. ZAPITRA mantém o resultado no registro de mensagem existente até que o consumidor retire um evento. É uma hipótese do modelo para revisão.
2. **Limite de `Lent` (opção 2 selecionada):** a guarda fica limitada ao tempo de trabalho. Se for descartada antes da materialização do evento terminal, o modelo cancela o trabalho ainda não iniciado e aguarda os leitores ativos. Quando o evento `Hashed` é materializado, todos os leitores já terminaram; a guarda pode ser liberada mesmo que o armazenamento limitado ainda não tenha permitido que `next_event()` entregue o evento. Esta é a interpretação explícita de ZAPITRA para “antes do evento da mensagem”; convidamos os mantenedores a confirmá-la.
3. **Unidades de worker em Lent:** duas unidades abstratas tornam observáveis os estados de cancelamento parcial. Elas são marcadores do scheduler do modelo, não uma afirmação sobre a quantidade de tarefas internas do BLAKE3.
4. **Tags e IDs limitados:** as tags são representadas por valores `u8`. Valores repetidos são permitidos. IDs internos de mensagem são separados.
5. **Capacidades positivas:** o modelo exige `limits.messages >= 1` e `limits.rooms >= 1`. Capacidades iguais a zero estão fora desta versão.
6. **Fora do escopo:** política de `length_hint`, destruição de Pipeline com trabalho em andamento, scheduler real, filas de conclusão do kernel e orçamento compartilhado entre várias instâncias de Pipeline.

As hipóteses estão explícitas para que os mantenedores possam corrigir o modelo. Elas não são apresentadas como defeitos do BLAKE3.

## O que ZAPITRA não afirma

- Não possui adaptador de runtime para uma implementação funcional de `Pipeline`.
- Não testa digests contra o runtime proposto.
- Não mede alocações, zero-copy, throughput ou latência.
- Não prova segurança de memória, ausência de data races ou soundness do contrato `unsafe fn lent` upstream.
- Não implementa `io_uring` nem registro de buffers fixos.
- Não substitui testes upstream, Shuttle, Miri, mutation testing ou revisão dos mantenedores.

## Estrutura

```text
src/model.rs       Modelo de estados e invariantes
src/explorer.rs    BFS limitado e oráculos de espera dos workers
src/main.rs        CLI e verificações fixas
tests/             Local para futuros testes de integração
docs/              Matriz de contratos, hipóteses, revisão e publicação
scripts/           Compilação, verificação, probes upstream e publicação
.github/workflows/ CI
```

## Licença

MIT. ZAPITRA é uma ferramenta comunitária independente. Não é afiliada nem endossada pelos mantenedores do BLAKE3 ou pelo programa Zcash Community Grants.
## Teste de consumidor da API upstream

Execute `./scripts/check-upstream-client.sh` para compilar um pequeno consumidor Rust contra o esqueleto fixado da API Pipeline do BLAKE3. O teste é somente de compilação: os métodos upstream usam `todo!()` e não executa hashing nem valida comportamento em tempo de execução.
