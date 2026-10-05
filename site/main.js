const themeButton = document.querySelector('#theme');
const themes = ['system', 'dark', 'light'];
let currentTheme = 'system';

function applyTheme(theme) {
  currentTheme = theme;
  document.documentElement.dataset.theme = theme;
  const next = themes[(themes.indexOf(theme) + 1) % themes.length];
  themeButton.setAttribute('aria-label', `Theme: ${theme}. Switch to ${next}.`);
  themeButton.title = `Theme: ${theme}. Switch to ${next}.`;
}

try {
  const saved = localStorage.getItem('appshell-site-theme');
  if (themes.includes(saved)) applyTheme(saved);
} catch {
  // System appearance still works when browser storage is unavailable.
}

themeButton.hidden = false;
themeButton.addEventListener('click', () => {
  applyTheme(themes[(themes.indexOf(currentTheme) + 1) % themes.length]);
  try {
    localStorage.setItem('appshell-site-theme', currentTheme);
  } catch {
    // The selection remains active for this page without persistence.
  }
});

if (navigator.clipboard?.writeText) {
  for (const button of document.querySelectorAll('[data-copy]')) {
    button.hidden = false;
    button.addEventListener('click', async () => {
      const code = document.getElementById(button.dataset.copy);
      const status = document.querySelector('#copy-status');
      try {
        await navigator.clipboard.writeText(code.textContent.trim());
        status.textContent = 'Setup commands copied to clipboard.';
        button.textContent = 'Copied!';
      } catch {
        status.textContent = 'Could not copy. Select the commands and copy them manually.';
        button.textContent = 'Select to copy';
      }
      window.setTimeout(() => {
        button.textContent = 'Copy commands';
      }, 2500);
    });
  }
}
