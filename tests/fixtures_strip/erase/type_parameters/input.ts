function fn<T>(a: T) {}
class C<T, U extends T = T> {}
const g = <T>(a: T) => a;
class D {
	m<T>(a: T) {}
}
