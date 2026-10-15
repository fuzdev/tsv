function fn(a: T): T;
function fn(a: U): U;
function fn(a) {
	return a;
}
class C {
	m(a: T): void;
	m(a) {}
}
