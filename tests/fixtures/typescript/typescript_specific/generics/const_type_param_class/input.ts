class Container<const T> {
	value: T;

	constructor(value: T) {
		this.value = value;
	}
}

class Wrapper<const T, const U> {
	first: T;
	second: U;

	constructor(first: T, second: U) {
		this.first = first;
		this.second = second;
	}
}
