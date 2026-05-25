import { writable } from 'svelte/store';

export type Theme = 'dark' | 'light';

export const theme = writable<Theme>('dark');

export function initTheme() {
  if (typeof window === 'undefined') return;

  // 1. Check local storage
  const savedTheme = localStorage.getItem('tagstudio-theme') as Theme | null;
  if (savedTheme === 'dark' || savedTheme === 'light') {
    applyTheme(savedTheme);
    return;
  }

  // 2. Fall back to system preferences
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  const systemTheme: Theme = prefersDark ? 'dark' : 'light';
  applyTheme(systemTheme);

  // 3. Listen to system preference changes dynamically
  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
  const handleChange = (e: MediaQueryListEvent) => {
    // Only apply if the user hasn't set a manual override
    if (!localStorage.getItem('tagstudio-theme')) {
      applyTheme(e.matches ? 'dark' : 'light');
    }
  };
  
  // Support older browsers
  if (mediaQuery.addEventListener) {
    mediaQuery.addEventListener('change', handleChange);
  } else {
    mediaQuery.addListener(handleChange);
  }
}

export function applyTheme(t: Theme) {
  if (typeof window === 'undefined') return;
  
  theme.set(t);
  const root = document.documentElement;
  if (t === 'light') {
    root.classList.add('light');
  } else {
    root.classList.remove('light');
  }
}

export function toggleTheme() {
  if (typeof window === 'undefined') return;

  theme.update((current) => {
    const next: Theme = current === 'dark' ? 'light' : 'dark';
    localStorage.setItem('tagstudio-theme', next);
    applyTheme(next);
    return next;
  });
}
