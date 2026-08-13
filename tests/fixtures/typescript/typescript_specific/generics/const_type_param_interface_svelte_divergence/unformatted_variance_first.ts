interface Single<const T> {
	value: T;
}

interface Multiple<const T, const U> {
	first: T;
	second: U;
}

interface WithVariance<const in T> {
	consume(value: T): void;
}

interface VarianceFirst<in const T> {
	consume(value: T): void;
}
