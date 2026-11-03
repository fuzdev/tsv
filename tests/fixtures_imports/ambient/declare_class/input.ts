@d(import('./d.js'))
declare class C {
	[import('./a.js')]: number;
	m() {
		return import('./b.js');
	}
}
