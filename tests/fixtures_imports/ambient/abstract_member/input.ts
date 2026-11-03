abstract class C {
	abstract m(): void;
	n() {
		return import('./a.js');
	}
}
