export type Language = 'zh' | 'en';
export type Translator = (chinese: string, english?: string) => string;
export const LANGUAGE_STORAGE_KEY = 'agent-usage-monitor.language';
export const localeFor = (language: Language) => language === 'zh' ? 'zh-CN' : 'en-US';
export const translate = (language: Language, chinese: string, english?: string) => language === 'en' ? english ?? chinese : chinese;
