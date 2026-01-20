import { useEffect, useMemo, useState } from 'react';
import {
  askAssistant,
  getArticle,
  getMemo,
  initializeApp,
  listDocuments,
  searchArticles,
  searchByNumber,
  searchMemos,
  updateFromSources
} from './lib/api';
import type {
  ArticleDetail,
  ArticleSummary,
  AssistantAnswer,
  DocumentSummary,
  MemoDetail,
  MemoSummary
} from './lib/api';

const TABS = ['Законы', 'Ассистент', 'Памятки'] as const;

type Tab = (typeof TABS)[number];

const SOURCE_KEY = 'majestic-law-sources';

function readStoredSources(): string[] {
  const stored = localStorage.getItem(SOURCE_KEY);
  if (!stored) return [];
  return stored
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
}

function storeSources(sources: string[]): void {
  localStorage.setItem(SOURCE_KEY, sources.join('\n'));
}

export default function App() {
  const [activeTab, setActiveTab] = useState<Tab>('Законы');
  const [sources, setSources] = useState<string[]>(readStoredSources());
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [lawsQuery, setLawsQuery] = useState('');
  const [articleNumber, setArticleNumber] = useState('');
  const [articleResults, setArticleResults] = useState<ArticleSummary[]>([]);
  const [selectedArticle, setSelectedArticle] = useState<ArticleDetail | null>(null);
  const [memosQuery, setMemosQuery] = useState('');
  const [memoResults, setMemoResults] = useState<MemoSummary[]>([]);
  const [selectedMemo, setSelectedMemo] = useState<MemoDetail | null>(null);
  const [assistantQuery, setAssistantQuery] = useState('');
  const [assistantAnswer, setAssistantAnswer] = useState<AssistantAnswer | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    const run = async () => {
      try {
        setStatus('Проверяем локальную базу...');
        await initializeApp(sources);
        setStatus(null);
        const docs = await listDocuments();
        setDocuments(docs);
      } catch (error) {
        setStatus('Не удалось инициализировать базу. Проверьте источники.');
        console.error(error);
      }
    };
    run();
  }, []);

  const sourcesText = useMemo(() => sources.join('\n'), [sources]);

  const handleUpdateSources = async () => {
    const newSources = sources
      .map((item) => item.trim())
      .filter(Boolean);
    storeSources(newSources);
    try {
      setStatus('Загружаем и обновляем документы...');
      await updateFromSources(newSources);
      const docs = await listDocuments();
      setDocuments(docs);
      setStatus('Обновление завершено.');
      setTimeout(() => setStatus(null), 2000);
    } catch (error) {
      console.error(error);
      setStatus('Ошибка обновления. Проверьте доступ к форуму.');
    }
  };

  const handleArticleSearch = async () => {
    setSelectedArticle(null);
    const results = lawsQuery.trim()
      ? await searchArticles(lawsQuery.trim())
      : [];
    setArticleResults(results);
  };

  const handleNumberSearch = async () => {
    setSelectedArticle(null);
    const results = articleNumber.trim()
      ? await searchByNumber(articleNumber.trim())
      : [];
    setArticleResults(results);
  };

  const handleSelectArticle = async (articleId: number) => {
    const article = await getArticle(articleId);
    setSelectedArticle(article);
  };

  const handleMemoSearch = async () => {
    setSelectedMemo(null);
    const results = memosQuery.trim()
      ? await searchMemos(memosQuery.trim())
      : [];
    setMemoResults(results);
  };

  const handleSelectMemo = async (memoId: number) => {
    const memo = await getMemo(memoId);
    setSelectedMemo(memo);
  };

  const handleAssistant = async () => {
    setAssistantAnswer(null);
    if (!assistantQuery.trim()) return;
    const answer = await askAssistant(assistantQuery.trim());
    setAssistantAnswer(answer);
  };

  return (
    <div className="app">
      <header className="app__header">
        <div>
          <h1>Majestic Law Assistant</h1>
          <p>Локальный справочник законов и памяток для сотрудников.</p>
        </div>
        <div className="app__status">{status}</div>
      </header>

      <nav className="tabs">
        {TABS.map((tab) => (
          <button
            key={tab}
            className={tab === activeTab ? 'tab tab--active' : 'tab'}
            onClick={() => setActiveTab(tab)}
            type="button"
          >
            {tab}
          </button>
        ))}
      </nav>

      {activeTab === 'Законы' && (
        <section className="panel">
          <div className="panel__grid">
            <div className="panel__left">
              <div className="card">
                <h2>Источники законодательной базы</h2>
                <p>Укажите ссылки на разделы форума Majestic (одна ссылка на строку).</p>
                <textarea
                  value={sourcesText}
                  onChange={(event) => setSources(event.target.value.split('\n'))}
                  placeholder="https://forum.majestic..."
                />
                <button type="button" onClick={handleUpdateSources}>
                  Скачать / обновить
                </button>
              </div>
              <div className="card">
                <h2>Поиск по словам</h2>
                <div className="form-row">
                  <input
                    value={lawsQuery}
                    onChange={(event) => setLawsQuery(event.target.value)}
                    placeholder="Например: задержание, оружие"
                  />
                  <button type="button" onClick={handleArticleSearch}>
                    Найти
                  </button>
                </div>
              </div>
              <div className="card">
                <h2>Поиск по номеру статьи</h2>
                <div className="form-row">
                  <input
                    value={articleNumber}
                    onChange={(event) => setArticleNumber(event.target.value)}
                    placeholder="ст. 12.3"
                  />
                  <button type="button" onClick={handleNumberSearch}>
                    Открыть
                  </button>
                </div>
              </div>
              <div className="card">
                <h2>Документы</h2>
                <ul className="list">
                  {documents.map((doc) => (
                    <li key={doc.id}>
                      <div className="list__title">{doc.title}</div>
                      <div className="list__meta">Обновлено: {doc.updatedAt}</div>
                    </li>
                  ))}
                </ul>
              </div>
            </div>

            <div className="panel__right">
              <div className="card">
                <h2>Результаты</h2>
                {articleResults.length === 0 && <p>Результатов пока нет.</p>}
                <ul className="list list--interactive">
                  {articleResults.map((article) => (
                    <li key={article.id}>
                      <button
                        type="button"
                        onClick={() => handleSelectArticle(article.id)}
                      >
                        <div className="list__title">
                          {article.number} {article.title}
                        </div>
                        <div className="list__meta">{article.documentTitle}</div>
                        <p className="list__snippet">{article.snippet}</p>
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
              <div className="card">
                <h2>Просмотр статьи</h2>
                {selectedArticle ? (
                  <div className="detail">
                    <h3>
                      {selectedArticle.number} {selectedArticle.title}
                    </h3>
                    <div className="detail__meta">
                      {selectedArticle.documentTitle}
                    </div>
                    <p className="detail__body">{selectedArticle.body}</p>
                  </div>
                ) : (
                  <p>Выберите статью из списка слева.</p>
                )}
              </div>
            </div>
          </div>
        </section>
      )}

      {activeTab === 'Ассистент' && (
        <section className="panel">
          <div className="card">
            <h2>Юридический ассистент</h2>
            <p>
              Ассистент сначала ищет релевантные статьи и памятки локально,
              затем формирует ответ с обязательными источниками.
            </p>
            <div className="form-row">
              <input
                value={assistantQuery}
                onChange={(event) => setAssistantQuery(event.target.value)}
                placeholder="Опишите ситуацию, например: можно ли применить силу?"
              />
              <button type="button" onClick={handleAssistant}>
                Ответить
              </button>
            </div>
            {assistantAnswer && (
              <div className="assistant">
                <p>{assistantAnswer.answer}</p>
                <div className="assistant__sources">
                  <h3>Источники</h3>
                  <ul>
                    {assistantAnswer.sources.map((source, index) => (
                      <li key={`${source.label}-${index}`}>{source.label}</li>
                    ))}
                  </ul>
                </div>
              </div>
            )}
          </div>
        </section>
      )}

      {activeTab === 'Памятки' && (
        <section className="panel">
          <div className="panel__grid">
            <div className="panel__left">
              <div className="card">
                <h2>Поиск памяток</h2>
                <div className="form-row">
                  <input
                    value={memosQuery}
                    onChange={(event) => setMemosQuery(event.target.value)}
                    placeholder="Например: остановка ТС, оформление"
                  />
                  <button type="button" onClick={handleMemoSearch}>
                    Найти
                  </button>
                </div>
              </div>
              <div className="card">
                <h2>Результаты</h2>
                <ul className="list list--interactive">
                  {memoResults.map((memo) => (
                    <li key={memo.id}>
                      <button type="button" onClick={() => handleSelectMemo(memo.id)}>
                        <div className="list__title">{memo.title}</div>
                        <p className="list__snippet">{memo.snippet}</p>
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            </div>
            <div className="panel__right">
              <div className="card">
                <h2>Просмотр памятки</h2>
                {selectedMemo ? (
                  <div className="detail">
                    <h3>{selectedMemo.title}</h3>
                    <p className="detail__body">{selectedMemo.body}</p>
                    <div className="detail__meta">Ссылки на статьи:</div>
                    <ul>
                      {selectedMemo.linkedArticles.map((article) => (
                        <li key={article.id}>
                          {article.number} {article.title}
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : (
                  <p>Выберите памятку из списка слева.</p>
                )}
              </div>
            </div>
          </div>
        </section>
      )}
    </div>
  );
}
