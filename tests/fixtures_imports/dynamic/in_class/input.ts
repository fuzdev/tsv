class C {
	field = import('./a.js');
	method() {
		return import('./b.js');
	}
	static {
		import('./c.js');
	}
}
