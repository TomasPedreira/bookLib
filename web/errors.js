const apiErrorTranslations = {
  'Não podes remover o número de páginas de um livro com leituras em páginas': 'Keep the page count for a book with reading sessions tracked in pages',
  'O número de páginas não pode ser inferior ao progresso registado': 'Page count cannot be lower than recorded progress',
  'Erro na base de dados': 'Database error',
  'Registo não encontrado': 'Entry not found',
  'Catálogo temporariamente indisponível': 'Catalog temporarily unavailable',
  'O catálogo não respondeu à pesquisa': 'The catalog did not respond to the search',
  'Resposta inválida do catálogo': 'Invalid catalog response',
  'Pesquisa entre 2 e 120 caracteres': 'Search must contain 2 to 120 characters',
  'Identificador de obra inválido': 'Invalid work ID',
  'Indica o título': 'Enter a title',
  'Título ou autor demasiado longo': 'Title or author is too long',
  'Número de páginas inválido': 'Invalid page count',
  'Classificação inválida': 'Invalid rating',
  'Identificador de edição inválido': 'Invalid edition ID',
  'ISBN inválido': 'Enter a valid ISBN-10 or ISBN-13',
  'Edição inválida no catálogo': 'Invalid edition data from the catalog',
  'Edição não encontrada no catálogo': 'No edition found for this ISBN',
  'Campo do livro demasiado longo': 'Book field is too long',
  'URL da capa inválido': 'Invalid cover URL',
  'Texto demasiado longo': 'Text is too long',
  'Esta edição já está na biblioteca': 'This edition is already in your library',
  'Unidade inválida': 'Invalid progress unit',
  'Indica o número de páginas ou usa percentagem': 'Enter a page count or use percentage progress',
  'Já existe uma leitura em curso': 'A reading session is already in progress',
  'Estado inválido': 'Invalid status',
  'Esta leitura já terminou; inicia uma releitura': 'This reading session has ended; start a new one',
  'Só podes alterar a leitura mais recente': 'You can only change the latest reading session',
  'Data inválida': 'Invalid date',
  'Esta leitura já terminou': 'This reading session has ended',
  'Versão de ficheiro não suportada': 'Unsupported backup version',
  'Ficheiro demasiado grande': 'Backup file is too large',
  'Ficheiro contém um livro sem título': 'Backup contains a book without a title',
  'Dados inválidos no ficheiro': 'Backup contains invalid data',
  'Escolhe pelo menos um género': 'Choose at least one genre'
};
export function apiErrorMessage(message, status) {
  if (!message) return `Error ${status}`;
  const range = /^Indica um valor entre 0 e (\d+)$/.exec(message);
  return range ? `Enter a value between 0 and ${range[1]}` : apiErrorTranslations[message] || message;
}
