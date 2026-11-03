class C {
	[k: string]: unknown;
	m() {
		return import('./m.js');
	}
}
