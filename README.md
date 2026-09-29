# Entrelinhas

Biblioteca pessoal para guardar livros, acompanhar leituras e descobrir edições no catálogo gratuito da Open Library. O servidor é escrito em Rust com Tokio e Axum; a interface usa HTML, CSS e JavaScript puro.

## Executar

1. Instala [Rust](https://rustup.rs/) se ainda não o tens. O ficheiro `rust-toolchain.toml` seleciona a versão necessária apenas para este projeto.
2. Na pasta do projeto, executa `cargo run --release`.
3. Abre <http://127.0.0.1:3000> no navegador.

Na primeira execução, o Cargo transfere as dependências e compila a aplicação. Os dados são guardados em `booklib.sqlite` nesta pasta. Pesquisa no catálogo e capas requerem acesso à internet; os livros já guardados continuam disponíveis sem internet.

## Utilização

- **Adicionar livro:** abre diretamente a descoberta automática. Escreve título, autor ou ISBN, escolhe uma edição e adiciona-a à biblioteca. O registo manual fica disponível se o catálogo não encontrar o livro.
- **Biblioteca:** pesquisa, filtra, classifica, anota e edita os livros.
- **Plan to read:** o botão "Prices" pesquisa pelo título do livro na secção de livros da Amazon.es. A pesquisa abre numa nova aba; confirma que a edição e o custo total com envio correspondem ao livro pretendido.
- **For you:** a página inicial mostra dez sugestões da Open Library, combinando popularidade, classificações e os géneros dos livros cujos detalhes abriste no BookLib. Se ainda não houver histórico, mostra escolhas populares. Exclui livros já guardados.
- **Browse:** explora mais sugestões, pesquisa livros ou autores à direita do título e filtra as sugestões por até quatro géneros e vários idiomas. Sem filtros, mostra livros populares. Podes escolher uma edição e adicioná-la à biblioteca. Filtros e histórico de livros vistos ficam guardados neste navegador.
- **Leituras:** inicia uma leitura, regista páginas ou percentagem, pausa, conclui ou abandona. Uma nova leitura do mesmo livro preserva o histórico anterior.
- **Os meus dados:** descarrega um JSON de cópia e restaura-o quando necessário. Restaurar substitui a biblioteca atual.

O servidor escuta apenas em `127.0.0.1` por defeito. Para alterar a porta, define `BOOKLIB_ADDR`, por exemplo `127.0.0.1:4000`. `BOOKLIB_DB` permite escolher outro ficheiro SQLite. Não exponhas a aplicação a uma rede sem acrescentar autenticação.

## Estrutura

- `src/library.rs`: API da biblioteca, leituras, progresso, estatísticas e cópias.
- `src/catalog.rs`: integração com Open Library, limite de pedidos e cache temporária.
- `migrations/`: esquema SQLite.
- `web/`: interface sem framework.

Os dados de livros e capas vêm da [Open Library](https://openlibrary.org/developers/api) quando escolhes uma edição. A aplicação guarda localmente uma cópia dos metadados selecionados.
