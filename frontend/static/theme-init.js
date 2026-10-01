// Apply the saved theme before first paint to avoid a flash of the
// default theme. Mirrors resolveTheme(); fully guarded so storage
// errors fall back to the static data-theme/data-mode on <html>.
(function () {
	try {
		var families = ['console', 'editorial', 'terminal', 'daw'];
		var darkNative = { terminal: 1, daw: 1 };
		var fam = localStorage.getItem('tunewright-theme-family');
		var mode = localStorage.getItem('tunewright-theme-mode');
		var legacy = localStorage.getItem('tunewright-theme');
		if (families.indexOf(fam) < 0) fam = 'console';
		if (mode !== 'dark' && mode !== 'light') {
			if (legacy === 'dark' || legacy === 'light') mode = legacy;
			else mode = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
		}
		if (darkNative[fam]) mode = 'dark';
		var r = document.documentElement;
		r.setAttribute('data-theme', fam);
		r.setAttribute('data-mode', mode);
	} catch (e) {
		/* keep the static default attributes */
	}
})();
