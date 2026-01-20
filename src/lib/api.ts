import { invoke } from '@tauri-apps/api/tauri';

export type DocumentSummary = {
  id: number;
  title: string;
  url: string | null;
  updatedAt: string;
};

export type ArticleSummary = {
  id: number;
  number: string;
  title: string;
  snippet: string;
  documentTitle: string;
};

export type ArticleDetail = {
  id: number;
  number: string;
  title: string;
  body: string;
  documentTitle: string;
};

export type MemoSummary = {
  id: number;
  title: string;
  snippet: string;
};

export type MemoDetail = {
  id: number;
  title: string;
  body: string;
  linkedArticles: { id: number; number: string; title: string }[];
};

export type AssistantAnswer = {
  answer: string;
  sources: { label: string; articleId?: number; memoId?: number }[];
};

export async function initializeApp(sources: string[]): Promise<void> {
  await invoke('initialize_app', { sources });
}

export async function updateFromSources(sources: string[]): Promise<void> {
  await invoke('update_from_sources', { sources });
}

export async function listDocuments(): Promise<DocumentSummary[]> {
  return invoke('list_documents');
}

export async function searchArticles(query: string): Promise<ArticleSummary[]> {
  return invoke('search_articles', { query });
}

export async function searchByNumber(number: string): Promise<ArticleSummary[]> {
  return invoke('search_by_number', { number });
}

export async function getArticle(articleId: number): Promise<ArticleDetail> {
  return invoke('get_article', { articleId });
}

export async function searchMemos(query: string): Promise<MemoSummary[]> {
  return invoke('search_memos', { query });
}

export async function getMemo(memoId: number): Promise<MemoDetail> {
  return invoke('get_memo', { memoId });
}

export async function askAssistant(query: string): Promise<AssistantAnswer> {
  return invoke('assistant_answer', { query });
}
