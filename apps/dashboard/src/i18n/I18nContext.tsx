import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { LANGUAGE_STORAGE_KEY, localeFor, translate, type Language, type Translator } from './messages';

export function readLanguage(storage?: Pick<Storage, 'getItem'>): Language {
  try {
    const saved = (storage ?? localStorage).getItem(LANGUAGE_STORAGE_KEY);
    return saved === 'en' ? 'en' : 'zh';
  } catch { return 'zh'; }
}

interface I18nValue { language: Language; locale: string; setLanguage: (language: Language) => void; t: Translator }
const I18nContext = createContext<I18nValue>({
  language: 'zh', locale: 'zh-CN', setLanguage: () => {}, t: (key, english) => translate('zh', key, english),
});
export function I18nProvider({ children, initialLanguage }: { children: ReactNode; initialLanguage?: Language }) {
  const [language, setLanguage] = useState<Language>(() => initialLanguage ?? readLanguage());
  useEffect(() => {
    document.documentElement.lang = localeFor(language);
    document.title = 'Agent Usage Monitor';
    try { localStorage.setItem(LANGUAGE_STORAGE_KEY, language); } catch { /* Switching still works when preference storage is unavailable. */ }
  }, [language]);
  const value = useMemo<I18nValue>(() => ({ language, locale: localeFor(language), setLanguage,
    t: (key, english) => translate(language, key, english) }), [language]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}
export function useI18n() { return useContext(I18nContext); }
