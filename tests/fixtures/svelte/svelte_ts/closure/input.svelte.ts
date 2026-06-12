export function fn() {
	let a = $state(0);
	let b = $derived(a * 2);

	return {
		get a() {
			return a;
		},
		get b() {
			return b;
		},
		inc() {
			a++;
		},
	};
}

export function fn2<T>(init: T) {
	let value = $state(init);

	$effect(() => {
		console.log(value);
	});

	return {
		get value() {
			return value;
		},
		set(v: T) {
			value = v;
		},
	};
}
